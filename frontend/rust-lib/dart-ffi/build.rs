fn main() {
  // The panic hook in src/lib.rs (log_panic_to_logcat) mirrors Rust panics
  // into the Android logcat via __android_log_print. liblog.so ships in the
  // NDK sysroot for every android target, so this is safe for cargo-ndk
  // cross builds; without it the UND __android_log_print reference could
  // fail to resolve at dlopen() time.
  if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
    println!("cargo:rustc-link-lib=dylib=log");
  }
  flowy_codegen::protobuf_file::dart_gen(env!("CARGO_PKG_NAME"));
}
