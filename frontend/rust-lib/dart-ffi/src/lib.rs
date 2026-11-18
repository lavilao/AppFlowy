#![allow(clippy::not_unsafe_ptr_arg_deref)]

use allo_isolate::Isolate;
use futures::ready;
use lazy_static::lazy_static;
use semver::Version;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Once, RwLock};
use std::task::{Context, Poll};
use std::{ffi::CStr, os::raw::c_char};
use tokio::sync::mpsc;
use tokio::task::LocalSet;
use tracing::{debug, error, info, trace, warn};

use flowy_core::config::AppFlowyCoreConfig;
use flowy_core::*;
use flowy_notification::{register_notification_sender, unregister_all_notification_sender};
use flowy_server_pub::AuthenticatorType;
use lib_dispatch::prelude::ToBytes;
use lib_dispatch::prelude::*;
use lib_dispatch::runtime::AFPluginRuntime;
use lib_log::stream_log::StreamLogSender;

use crate::appflowy_yaml::save_appflowy_cloud_config;
use crate::env_serde::AppFlowyDartConfiguration;
use crate::notification::DartNotificationSender;
use crate::{
  c::{extend_front_four_bytes_into_bytes, forget_rust},
  model::{FFIRequest, FFIResponse},
};

mod appflowy_yaml;
mod c;
mod env_serde;
mod model;
mod notification;
mod protobuf;

// ---------------------------------------------------------------------------
// Panic visibility (Android). A Rust panic inside these extern "C" entry
// points aborts the process — SIGABRT, reported by `dumpsys activity
// exit-info` as reason=5 (APP CRASH(NATIVE)) status=6 — but the default panic
// hook only prints to stderr, and a release Android build sends stderr to
// /dev/null. The result is a native crash with NOTHING in logcat (the Redmi
// 9A startup crash observed after the libc++_shared.so dlopen fix: dlopen
// succeeds, then the process dies with status=6 and no output at all).
//
// This hook mirrors every panic into the Android logcat under the tag
// "RustPanic" (via liblog's __android_log_print, linked by build.rs) and to
// stderr elsewhere, so the exact panic location and message become visible:
//   adb logcat -s RustPanic
// It is installed from every FFI entry point (Once-guarded, so the earliest
// call — set_stream_port, before init_sdk — wins).
// ---------------------------------------------------------------------------
static INSTALL_PANIC_HOOK: Once = Once::new();

pub fn install_panic_logger() {
  INSTALL_PANIC_HOOK.call_once(|| {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
      let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "<unknown>".to_string());
      let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
        (*s).to_string()
      } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
      } else {
        "<non-string panic payload>".to_string()
      };
      let msg = format!("AppFlowy Rust panic at {} — {}", location, payload);
      eprintln!("{}", msg);
      #[cfg(target_os = "android")]
      log_panic_to_logcat(&msg);
      default_hook(info);
    }));
  });
}

#[cfg(target_os = "android")]
fn log_panic_to_logcat(msg: &str) {
  use std::ffi::CString;

  // liblog.so is a public Android library (/system/public.libraries.txt) and
  // is linked for android targets by build.rs (`cargo:rustc-link-lib=dylib=log`).
  extern "C" {
    fn __android_log_print(prio: i32, tag: *const c_char, fmt: *const c_char, ...) -> i32;
  }

  // logcat truncates a single entry at ~4068 bytes; clamp to a UTF-8 boundary
  // so CString::new never fails on a cut multi-byte character.
  let mut end = msg.len().min(3500);
  while end > 0 && !msg.is_char_boundary(end) {
    end -= 1;
  }
  let text = match CString::new(&msg[..end]) {
    Ok(text) => text,
    Err(_) => CString::new("AppFlowy Rust panic (payload contained NUL)").unwrap(),
  };
  let tag = CString::new("RustPanic").unwrap();
  unsafe {
    __android_log_print(
      6, // ANDROID_LOG_ERROR
      tag.as_ptr(),
      b"%s\0".as_ptr() as *const c_char,
      text.as_ptr(),
    );
  }
}

lazy_static! {
  static ref DART_APPFLOWY_CORE: DartAppFlowyCore = DartAppFlowyCore::new();
  static ref LOG_STREAM_ISOLATE: RwLock<Option<Isolate>> = RwLock::new(None);
}

pub struct Task {
  dispatcher: Arc<AFPluginDispatcher>,
  request: AFPluginRequest,
  port: i64,
  ret: Option<mpsc::Sender<AFPluginEventResponse>>,
}

unsafe impl Send for Task {}
unsafe impl Sync for DartAppFlowyCore {}

struct DartAppFlowyCore {
  core: Arc<RwLock<Option<AppFlowyCore>>>,
  handle: RwLock<Option<std::thread::JoinHandle<()>>>,
  sender: RwLock<Option<mpsc::UnboundedSender<Task>>>,
}

impl DartAppFlowyCore {
  fn new() -> Self {
    Self {
      #[allow(clippy::arc_with_non_send_sync)]
      core: Arc::new(RwLock::new(None)),
      handle: RwLock::new(None),
      sender: RwLock::new(None),
    }
  }

  fn dispatcher(&self) -> Option<Arc<AFPluginDispatcher>> {
    let binding = self
      .core
      .read()
      .expect("Failed to acquire read lock for core");
    let core = binding.as_ref();
    core.map(|core| core.event_dispatcher.clone())
  }

  fn dispatch(
    &self,
    request: AFPluginRequest,
    port: i64,
    ret: Option<mpsc::Sender<AFPluginEventResponse>>,
  ) {
    if let Ok(sender_guard) = self.sender.read() {
      let dispatcher = match self.dispatcher() {
        Some(dispatcher) => dispatcher,
        None => {
          error!("Failed to get dispatcher: dispatcher is None");
          return;
        },
      };

      if let Some(sender) = sender_guard.as_ref() {
        if let Err(e) = sender.send(Task {
          dispatcher,
          request,
          port,
          ret,
        }) {
          error!("Failed to send task: {}", e);
        }
      } else {
        error!("Failed to send task: sender is None");
      }
    } else {
      warn!("Failed to acquire read lock for sender");
    }
  }
}

#[no_mangle]
pub extern "C" fn init_sdk(_port: i64, data: *mut c_char) -> i64 {
  install_panic_logger();
  let c_str = unsafe {
    if data.is_null() {
      return -1;
    }
    CStr::from_ptr(data)
  };
  let serde_str = c_str
    .to_str()
    .expect("Failed to convert C string to Rust string");
  let configuration = AppFlowyDartConfiguration::from_str(serde_str);
  configuration.write_env();

  if configuration.authenticator_type == AuthenticatorType::AppFlowyCloud {
    let _ = save_appflowy_cloud_config(&configuration.root, &configuration.appflowy_cloud_config);
  }

  let mut app_version =
    Version::parse(&configuration.app_version).unwrap_or_else(|_| Version::new(0, 5, 8));

  let min_version = Version::new(0, 5, 8);
  if app_version < min_version {
    app_version = min_version;
  }

  let config = AppFlowyCoreConfig::new(
    app_version,
    configuration.custom_app_path,
    configuration.origin_app_path,
    configuration.device_id,
    configuration.platform,
    DEFAULT_NAME.to_string(),
  );

  if let Some(core) = &*DART_APPFLOWY_CORE.core.write().unwrap() {
    core.close_db();
  }

  let log_stream = LOG_STREAM_ISOLATE
    .write()
    .unwrap()
    .take()
    .map(|isolate| Arc::new(LogStreamSenderImpl { isolate }) as Arc<dyn StreamLogSender>);
  let (sender, task_rx) = mpsc::unbounded_channel::<Task>();
  let runtime = Arc::new(AFPluginRuntime::new().unwrap());
  let cloned_runtime = runtime.clone();
  let handle = std::thread::spawn(move || {
    let local_set = LocalSet::new();
    cloned_runtime.block_on(local_set.run_until(Runner { rx: task_rx }));
  });

  *DART_APPFLOWY_CORE.sender.write().unwrap() = Some(sender);
  *DART_APPFLOWY_CORE.handle.write().unwrap() = Some(handle);
  let cloned_runtime = runtime.clone();
  *DART_APPFLOWY_CORE.core.write().unwrap() = runtime
    .block_on(async move { Some(AppFlowyCore::new(config, cloned_runtime, log_stream).await) });
  0
}

#[no_mangle]
#[allow(clippy::let_underscore_future)]
pub extern "C" fn async_event(port: i64, input: *const u8, len: usize) {
  install_panic_logger();
  let request: AFPluginRequest = FFIRequest::from_u8_pointer(input, len).into();
  #[cfg(feature = "verbose_log")]
  trace!(
    "[FFI]: {} Async Event: {:?} with {} port",
    &request.id,
    &request.event,
    port
  );

  DART_APPFLOWY_CORE.dispatch(request, port, None);
}

/// A persistent future that processes [Arbiter] commands.
struct Runner {
  rx: mpsc::UnboundedReceiver<Task>,
}

impl Future for Runner {
  type Output = ();

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    loop {
      match ready!(self.rx.poll_recv(cx)) {
        None => return Poll::Ready(()),
        Some(task) => {
          let Task {
            dispatcher,
            request,
            port,
            ret,
          } = task;

          tokio::task::spawn_local(async move {
            let resp = AFPluginDispatcher::boxed_async_send_with_callback(
              dispatcher.as_ref(),
              request,
              move |resp: AFPluginEventResponse| {
                #[cfg(feature = "verbose_log")]
                trace!("[FFI]: Post data to dart through {} port", port);
                Box::pin(post_to_flutter(resp, port))
              },
            )
            .await;

            if let Some(ret) = ret {
              let _ = ret.send(resp).await;
            }
          });
        },
      }
    }
  }
}

#[no_mangle]
pub extern "C" fn sync_event(_input: *const u8, _len: usize) -> *const u8 {
  install_panic_logger();
  error!("unimplemented sync_event");

  let response_bytes = vec![];
  let result = extend_front_four_bytes_into_bytes(&response_bytes);
  forget_rust(result)
}

#[no_mangle]
pub extern "C" fn set_stream_port(notification_port: i64) -> i32 {
  install_panic_logger();
  unregister_all_notification_sender();
  register_notification_sender(DartNotificationSender::new(notification_port));
  0
}

#[no_mangle]
pub extern "C" fn set_log_stream_port(port: i64) -> i32 {
  install_panic_logger();
  *LOG_STREAM_ISOLATE.write().unwrap() = Some(Isolate::new(port));
  0
}

#[inline(never)]
#[no_mangle]
pub extern "C" fn link_me_please() {}

#[inline(always)]
#[allow(clippy::blocks_in_conditions)]
async fn post_to_flutter(response: AFPluginEventResponse, port: i64) {
  let isolate = allo_isolate::Isolate::new(port);
  match isolate
    .catch_unwind(async {
      let ffi_resp = FFIResponse::from(response);
      ffi_resp.into_bytes().unwrap().to_vec()
    })
    .await
  {
    Ok(_) => {
      #[cfg(feature = "verbose_log")]
      trace!("[FFI]: Post data to dart success");
    },
    Err(err) => {
      error!("[FFI]: allo_isolate post failed: {:?}", err);
    },
  }
}

#[no_mangle]
pub extern "C" fn rust_log(level: i64, data: *const c_char) {
  install_panic_logger();
  if data.is_null() {
    error!("[flutter error]: null pointer provided to backend_log");
    return;
  }

  let log_result = unsafe { CStr::from_ptr(data) }.to_str();

  let log_str = match log_result {
    Ok(str) => str,
    Err(e) => {
      error!(
        "[flutter error]: Failed to convert C string to Rust string: {:?}",
        e
      );
      return;
    },
  };

  match level {
    0 => info!("[Flutter]: {}", log_str),
    1 => debug!("[Flutter]: {}", log_str),
    2 => trace!("[Flutter]: {}", log_str),
    3 => warn!("[Flutter]: {}", log_str),
    4 => error!("[Flutter]: {}", log_str),
    _ => warn!("[flutter error]: Unsupported log level: {}", level),
  }
}

#[no_mangle]
pub extern "C" fn set_env(_data: *const c_char) {
  install_panic_logger();
  // Deprecated
}

struct LogStreamSenderImpl {
  isolate: Isolate,
}
impl StreamLogSender for LogStreamSenderImpl {
  fn send(&self, message: &[u8]) {
    self.isolate.post(message.to_vec());
  }
}
