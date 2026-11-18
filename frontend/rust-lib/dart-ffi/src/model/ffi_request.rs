use bytes::Bytes;
use flowy_derive::ProtoBuf;
use lib_dispatch::prelude::AFPluginRequest;
use std::convert::TryFrom;

#[derive(Default, ProtoBuf)]
pub struct FFIRequest {
  #[pb(index = 1)]
  pub(crate) event: String,

  #[pb(index = 2)]
  pub(crate) payload: Vec<u8>,
}

impl FFIRequest {
  pub fn from_u8_pointer(pointer: *const u8, len: usize) -> Self {
    // This runs directly inside an `extern "C"` export, so a panic here can
    // never unwind: the Rust runtime aborts the process (SIGABRT). Guard the
    // decode so a null pointer, an empty or an absurd length (e.g. a garbage
    // value crossing the FFI boundary on a 32-bit target) degrades to an
    // empty request instead of crashing the app.
    if pointer.is_null() || len == 0 || len > isize::MAX as usize {
      return FFIRequest::default();
    }

    let buffer = unsafe { std::slice::from_raw_parts(pointer, len) }.to_vec();
    let bytes = Bytes::from(buffer);
    FFIRequest::try_from(bytes).unwrap_or_default()
  }
}

impl std::convert::From<FFIRequest> for AFPluginRequest {
  fn from(ffi_request: FFIRequest) -> Self {
    AFPluginRequest::new(ffi_request.event).payload(ffi_request.payload)
  }
}
