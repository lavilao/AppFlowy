# Running AppFlowy on 32-bit Android (armeabi-v7a)

**A debugging report and upstream integration guide**

- Device under test: **Redmi 9A** (MediaTek Helio G25), **Android 11**, `primaryCpuAbi=armeabi-v7a` — a 32-bit-only device with no 64-bit fallback.
- Code base: AppFlowy 0.11.x, Flutter 3.27.4, Rust 1.85.0, NDK r27d.
- Outcome: the app now installs, loads the Rust backend and runs on armeabi-v7a. Three independent bugs had to be fixed; each one was invisible (or silently different) on 64-bit builds.
- Commits (this fork, in order): `f9168cc`, `3c76c12`, `f481104`, `3f715a2`.

This document is written so the AppFlowy maintainers can (a) understand exactly why 32-bit devices crashed, and (b) adopt the small, generalizable pieces of this work upstream.

---

## 0. TL;DR

| # | Symptom on armv7 | Root cause | Fix |
|---|------------------|------------|-----|
| 1 | `dlopen failed: cannot locate symbol "_ZTTNSt6__ndk118basic_stringstream…"` before the first Flutter frame | The APK shipped a `libc++_shared.so` from **NDK r24** while `libdart_ffi.so` was linked by **NDK r27d**; 17 of its 119 undefined `std::__ndk1` symbols don't exist in the r24 runtime. Gradle/CMake let the **wrong** copy win the merge. | Ship the libc++ runtime of the *same* NDK that linked the Rust library; pin one NDK everywhere; add a post-build symbol check (`f9168cc`, `3c76c12`) |
| 2 | Native crash (`SIGABRT`) with **no logcat output at all** | After bug 1 was fixed, a Rust panic aborted the process; release builds send stderr to `/dev/null`, so the panic text was lost. | Install a Rust panic hook that mirrors panics into logcat (`RustPanic` tag) via `__android_log_print` (`f481104`) |
| 3 | `capacity overflow` panic in `library/alloc/src/slice.rs` → `panic in a function that cannot unwind` → `SIGABRT` | **FFI ABI mismatch**: Dart declares `async_event(..., Uint64 len)` but Rust exported `len: usize`. On armhf (32-bit), Dart passes the 64-bit length on the stack while Rust reads a garbage value from register `r3`. The garbage length makes `slice::from_raw_parts(ptr, garbage).to_vec()` overflow. On 64-bit, `usize == u64`, so this never reproduced. | Declare FFI lengths as `u64` (explicit width) on the Rust side and never panic inside an `extern "C"` export (`3f715a2`) |

The single most upstream-worthy lesson: **`usize` in a `#[no_mangle] pub extern "C"` signature called from Dart is a latent crash on every 32-bit target.** Dart's FFI `int` is always 64-bit; Rust's `usize` is pointer-width. They only agree on 64-bit platforms.

---

## 1. Environment and reproduction

```
Redmi 9A · Android 11 (MIUI) · armeabi-v7a only (no arm64 fallback)
adb shell getprop ro.product.cpu.abi      # armeabi-v7a
adb shell pm dump io.appflowy.appflowy | grep primaryCpuAbi
```

The app was built as a split ABI APK (`flutter build apk --split-per-abi`, `--target-platform android-arm`) and installed directly on the device. Every crash below reproduced deterministically at launch:

```
adb logcat -c
adb shell am force-stop io.appflowy.appflowy
adb shell am start -n io.appflowy.appflowy/.MainActivity
```

On Android 11+, `dumpsys activity exit-info` is the best first look when the process dies silently:

```
adb shell dumpsys activity exit-info io.appflowy.appflowy
```

---

## 2. Bug 1 — `libc++_shared.so` version skew kills `dlopen()`

### 2.1 Symptom

```
Invalid argument(s): Failed to load dynamic library 'libdart_ffi.so':
dlopen failed: cannot locate symbol
"_ZTTNSt6__ndk118basic_stringstreamIcNS_11char_traitsIcEENS_9allocatorIcEEEE"
```

No native crash record was produced — the Dart FFI threw before the process died, so `exit-info` stayed empty.

### 2.2 Why: four native libraries, four different NDks

Provenance audit of `lib/armeabi-v7a/` inside the APK:

| Library | Producer | NDK |
|---|---|---|
| `libdart_ffi.so` | Rust backend via cargo-ndk | **r27d** |
| `libc++_shared.so` | copied from `${ANDROID_NDK}` by `src/main/CMakeLists.txt` | **r24** (`ndkVersion 24.0.8215888`) |
| `libflutter.so` | prebuilt Flutter engine | r26d (statically links its own libc++) |
| `libdatastore_shared_counter.so` | androidx datastore AAR | r25c |

`libdart_ffi.so` contains **119 undefined `std::__ndk1` symbols** (Rust's `std` for Android and the bundled RocksDB C++ both resolve against NDK's libc++). **17 of them** — stringstream / stringbuf / filebuf vtables and VTTs like `_ZTTNSt6__ndk118basic_stringstreamIcNS_11char_traitsIcEENS_9allocatorIcEEEE` — are exported by *neither* the r24 `libc++_shared.so` packaged in the APK *nor* the device system runtime. `dlopen()` therefore fails on the first FFI touch (`rust_log` during logging init).

The r24 copy won because `CMakeLists.txt` copies `libc++_shared.so` from the `ndkVersion`-pinned NDK into `app/src/main/jniLibs/<abi>/`, **overwriting** the correct r27d copy that cargo-ndk had dropped there.

### 2.3 Fix (`f9168cc` + `3c76c12`)

1. **`android/app/build.gradle`**: `ndkVersion 24.0.8215888 → 27.3.13750724` (r27d) so the CMake copy ships the runtime the Rust library was actually linked against.
2. **`android/app/src/main/CMakeLists.txt`**: copy the runtime from
   `toolchains/llvm/prebuilt/<host>/sysroot/usr/lib/<triple>/libc++_shared.so`.
   This matters: **NDK r27 removed `sources/cxx-stl/llvm-libc++/libs/`**, so the old copy path breaks at configure time with r27d. The sysroot path has existed since r21 and is byte-identical content.
3. **`packagingOptions.jniLibs { pickFirsts += "**/libc++_shared.so" }`** so a duplicate runtime copy can never fail the merge again.
4. **CI** (`build-armeabiv7-glm.yml`): install the NDK explicitly (`sdkmanager "ndk;27.3.13750724"`) and export `ANDROID_NDK_HOME`/`NDK_HOME` so **cargo-ndk and gradle build against the exact same NDK** — no runner-image drift between the two.
5. Also: commented out an arm64-v8a-only `ndk.abiFilters` (it conflicts with `--split-per-abi` splits) and committed a stable release keystore so successive releases update in place when sideloading.

### 2.4 Regression guard: post-build symbol check

`frontend/scripts/check_apk_libcxx_symbols.sh` runs against the **built APK** and fails CI unless:

```
readelf --dyn-syms -W libc++_shared.so | grep -c stringstream   # must be > 0
comm -23 \
  <(readelf --dyn-syms -W libdart_ffi.so    | awk '$7=="UND"{print $8}' | sort -u) \
  <(readelf --dyn-syms -W libc++_shared.so  | awk '{print $8}' | sort -u | sed 's/@.*//')
# must be empty
```

Applied to the broken APK: stringstream count = **0**, diff = **17 symbols**. Applied to the fixed APK: stringstream count = **20**, diff = **empty** (all 380 UND symbols of `libdart_ffi.so` resolve).

> Upstream note: NDK's official libc++ policy is that a *newer* runtime is backward-compatible with libraries built by *older* NDks — never the reverse. When several producers contribute native libs to one APK, ship the **newest** `libc++_shared.so` and assert the symbol closure at build time.

---

## 3. Bug 2 — silent `SIGABRT`, zero diagnostics

### 3.1 Symptom

After bug 1 was fixed (v0.11.4), the app no longer died in `dlopen()` — but still died before the first frame:

```
adb shell dumpsys activity exit-info io.appflowy.appflowy
  reason=5 (APP CRASH(NATIVE))   status=6 (SIGABRT)   trace=null   state=empty
adb logcat | grep -E "E flutter|dlopen|cannot locate"    # EMPTY
```

### 3.2 Why logcat is empty

A Rust `panic!` unwinding across an `extern "C"` boundary is UB; the Rust runtime detects it, prints `panic in a function that cannot unwind`, and **aborts** the process. The default panic hook prints to **stderr**, and a release Android build sends stderr to `/dev/null`. Result: `SIGABRT` with no evidence anywhere. (`exit-info` records *that* the process was natively killed, but on-device `trace=null` — the tombstone/dropbox data wasn't retrievable without root.)

### 3.3 Fix (`f481104`): a panic hook that talks to logcat

`dart-ffi/src/lib.rs` installs an `std::panic::set_hook` from **every FFI entry point** (guarded by `Once`; `set_stream_port` is the first one Dart calls, before `init_sdk`). The hook formats panic location + payload and writes it to logcat under tag **`RustPanic`** via `__android_log_print`, then chains to the default hook:

```
adb logcat -s RustPanic        # exact Rust panic location + message on device
```

Two supporting details worth copying upstream:

- **`build.rs`** links the runtime library explicitly on Android targets — without it the `__android_log_print` reference would itself fail at `dlopen()`:
  ```rust
  if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
      println!("cargo:rustc-link-lib=dylib=log");
  }
  ```
  `liblog.so` ships in the NDK sysroot for every Android target and is listed in `/system/public_libraries.txt`, so linking against it is safe for any app.
- **Logcat truncates single entries at ~4068 bytes**; the hook clamps the message to a UTF-8 boundary before handing it to `CString::new`.

The same commit also made the two bare `unwrap()`s in the startup chain (`AppFlowyDartConfiguration::from_str`, KV store init) panic with the offending payload/path instead of an opaque `called Option::unwrap() on a None value`.

This turned the next crash from "invisible" into a one-line diagnosis.

---

## 4. Bug 3 — the real crash: FFI signature width mismatch (`usize` vs `Uint64`)

### 4.1 What the panic hook revealed

```
thread '<unnamed>' panicked at library/alloc/src/slice.rs:
capacity overflow
...
panic in a function that cannot unwind
→ SIGABRT
```

`capacity overflow` inside `alloc/src/slice.rs` means: **a slice was cloned/collected with a length whose byte-size exceeds `isize::MAX`** (≈ 2.1 GiB on armv7). The only place in the startup path that does that is the FFI request decoder:

```rust
// dart-ffi/src/model/ffi_request.rs (before)
pub fn from_u8_pointer(pointer: *const u8, len: usize) -> Self {
    let buffer = unsafe { std::slice::from_raw_parts(pointer, len) }.to_vec();
    ...
}
```

called from:

```rust
// dart-ffi/src/lib.rs (before)
pub extern "C" fn async_event(port: i64, input: *const u8, len: usize) { ... }
```

### 4.2 The ABI mismatch, precisely

The Dart binding declares the C symbol with a **64-bit** length:

```dart
// appflowy_backend/lib/ffi.dart
typedef _invoke_async_C = Void Function(
  Int64 port,
  Pointer<Uint8> input,
  Uint64 len,        // 8 bytes — always
);
```

while Rust exports it with a **pointer-width** length:

```rust
pub extern "C" fn async_event(port: i64, input: *const u8, len: usize)  // 4 bytes on armv7
```

On **armhf** (AAPCS, the ABI of `armeabi-v7a` Android builds) the argument placement differs:

| Argument | Dart (Uint64 len) | Rust (usize len) |
|---|---|---|
| `port: i64` | `r0:r1` (64-bit values start at even registers) | `r0:r1` ✔ |
| `input: ptr` | `r2` | `r2` ✔ |
| `len` | 64-bit value **cannot start at `r3`** (odd register) → `r3` is skipped as *padding*, value goes to the stack `[sp, #0..8]` | 32-bit value → **`r3`** |

So Rust reads `len` from **`r3` — a padding register the caller never wrote, containing whatever garbage the Dart runtime left there**. The actual length sits on the stack, unread. With a garbage `len` above `isize::MAX`, `slice::from_raw_parts(..).to_vec()` panics with `capacity overflow` in `alloc/src/slice.rs`; unwinding out of `extern "C"` converts it to `panic in a function that cannot unwind` → abort → `SIGABRT`.

On **64-bit** targets `usize` and `u64` are both 8 bytes, so both sides agree — desktop builds and arm64 phones never see any of this. That is exactly why this survived all CI and 64-bit device testing.

### 4.3 Fix (`3f715a2`)

```rust
// dart-ffi/src/lib.rs (after)
pub extern "C" fn async_event(port: i64, input: *const u8, len: u64) {
  install_panic_logger();
  // match the Dart `Uint64` declaration on every ABI; on 64-bit targets
  // `u64` and `usize` are ABI-identical, so nothing changes there
  let len = usize::try_from(len).unwrap_or(0);
  let request: AFPluginRequest = FFIRequest::from_u8_pointer(input, len).into();
  ...
}
```

`sync_event` (same mismatch, currently unimplemented) got the same treatment, and the decoder became panic-free — because it runs *inside* an `extern "C"` export, where a panic is always fatal:

```rust
// dart-ffi/src/model/ffi_request.rs (after)
pub fn from_u8_pointer(pointer: *const u8, len: usize) -> Self {
    if pointer.is_null() || len == 0 || len > isize::MAX as usize {
        return FFIRequest::default();
    }
    let buffer = unsafe { std::slice::from_raw_parts(pointer, len) }.to_vec();
    FFIRequest::try_from(Bytes::from(buffer)).unwrap_or_default()
}
```

The Dart side was intentionally **not** changed — `ffi.dart` already documents the correct 64-bit contract; the Rust side was the deviating party.

### 4.4 Verification

- `cargo check -p dart-ffi` (Rust 1.85) and `cargo fmt --check` pass.
- CI (`build-armeabiv7-glm.yml`) produced release **v0.11.6** from commit `3f715a2` (APK + SHA256 published on the release page).
- **End-to-end: v0.11.6 installed on the Redmi 9A — the app launches and runs.** No `RustPanic` entries, no `APP CRASH(NATIVE)` in `exit-info`.

---

## 5. Why 32-bit testing matters

Each of the three bugs was a "works on my (64-bit) machine" failure:

1. **Bug 1** only exists where `libc++_shared.so` is loaded as a *separate* runtime for a library built by a *different* NDK — a combination 64-bit builds can also produce, but which the armv7 device surfaced immediately.
2. **Bug 2** exists everywhere but is *invisible* without the panic hook; 64-bit testing simply hadn't hit a panic path.
3. **Bug 3** is *strictly* a 32-bit-only crash: pointer-width `usize` vs 64-bit Dart `int` in one signature. It cannot reproduce on arm64, x86_64, or any desktop target.

---

## 6. Recommended changes for upstream AppFlowy

All of these are small and generic; none of them change behavior on 64-bit platforms.

### 6.1 FFI hygiene: explicit-width types in every Rust export (the crash fix)

- Rule: in `#[no_mangle] pub extern "C"` functions called from Dart (`frontend/rust-lib/dart-ffi/src/lib.rs`), **never use `usize`/`isize` for parameters or return values**; use `u64`/`i64`/`u32`… to match the Dart `ffi.dart` typedefs, and convert at the boundary (`usize::try_from(...)`).
- Current audit: `async_event`/`sync_event` were the only mismatches — `init_sdk`, `set_stream_port`, `set_log_stream_port`, `rust_log`, `set_env` already agree with their Dart typedefs (Dart declares ports as `Int64`, and Rust takes `i64`).
- Cheap CI enforcement: a script that parses `extern "C"` signatures in `dart-ffi/src/lib.rs` and the corresponding `Function(...)` typedefs in `appflowy_backend/lib/ffi.dart` and fails on width mismatch (or, even better, an FFI round-trip unit test compiled and run for `armv7a-linux-androideabi`).

### 6.2 One NDK, one libc++ runtime

- Keep `ndkVersion` (gradle) = `NDK_VERSION` (CI env) = the NDK cargo-ndk uses, from a single source of truth. The fork pins `27.3.13750724` in all three places.
- `packagingOptions.jniLibs.pickFirsts` for `**/libc++_shared.so` so duplicate copies can never fail the merge.
- If the copy happens in CMake, note **NDK r27 removed `sources/cxx-stl/llvm-libc++/libs/`** — use `toolchains/llvm/prebuilt/<host>/sysroot/usr/lib/<triple>/libc++_shared.so`.
- Alternative that removes this bug class entirely: static-link the C++ runtime into `libdart_ffi.so` (e.g. `-C link-arg=-static-libstdc++` for Android Rust builds). RocksDB is the dependency that drags libc++ in; static linking costs ~1–2 MB of APK size but makes the Rust library self-contained and immune to runtime-version skew and to gradle merge order.

### 6.3 Fail the build on unresolved native symbols

- Run a post-build symbol-closure check for **every ABI** (see `frontend/scripts/check_apk_libcxx_symbols.sh`): every `UND` symbol of every packaged `.so` must resolve against something that is actually shipped (packaged `libc++_shared.so`, system public libraries) — and the packaged runtime must demonstrably be a modern libc++ (`stringstream` export count > 0). This check would have caught bug 1 before the first release.

### 6.4 Panic visibility in release builds

- Install the logcat panic hook from every FFI entry point (`RustPanic` tag, `Once`-guarded, `build.rs` links `dylib=log` on Android). ~50 lines total; it converts "silent SIGABRT, trace=null" into a one-line root cause. Keep `adb logcat -s RustPanic` in the bug-report template.

### 6.5 Add armv7 to CI (build + smoke test)

- Building for `armeabi-v7a` is enough to catch *link*-time issues, but bug 3 shows the FFI boundary needs a **runtime** check: a 32-bit emulator smoke test (e.g. `system-images;android-24;google_apis;arm-v7a`) that launches the app, or at minimum an armv7-compiled FFI round-trip test, would have caught both bug 2 and bug 3 pre-release.
- `flutter build apk --split-per-abi` conflicts with a pinned `ndk.abiFilters` — keep them in sync.

---

## 7. Timeline

| Date (UTC) | Commit | What | Result on Redmi 9A |
|---|---|---|---|
| Nov 18, 03:27 | `f9168cc` | Ship r27d `libc++_shared.so` (ndkVersion bump, CMake sysroot path, `pickFirsts`, CI NDK pinning, APK symbol check, release keystore, v0.11.4 release flow) | dlopen error gone → app still dies, now silently (`SIGABRT`) |
| Nov 18, 03:36 | `3c76c12` | CI robustness: sdkmanager not on PATH on current runner images | reproducible CI builds |
| Nov 18, 12:17 | `f481104` | Rust panic hook → logcat (`RustPanic`), `build.rs` links `liblog`, better panic payloads in startup chain, v0.11.5 | crash still present, but **now visible**: `capacity overflow` in `alloc/src/slice.rs` |
| Nov 18, 17:46 | `3f715a2` | `async_event`/`sync_event` `len: u64` (ABI match), panic-free `FFIRequest` decode, v0.11.6 | **app launches and runs** |

---

## 8. Appendix — command reference

```bash
# --- device reproduction ---------------------------------------------
adb logcat -c && adb shell am force-stop io.appflowy.appflowy
adb shell am start -n io.appflowy.appflowy/.MainActivity
adb logcat -d -b crash                      # native crash buffer (tombstone-level info)
adb shell dumpsys activity exit-info io.appflowy.appflowy
adb logcat -s RustPanic                     # Rust panic hook output (this fork)

# --- APK native-lib audit --------------------------------------------
unzip -o appflowy-armeabi-v7a.apk 'lib/armeabi-v7a/*' -d apk_extract/
readelf --dyn-syms -W apk_extract/lib/armeabi-v7a/libc++_shared.so \
  | grep -c stringstream                    # modern runtime? must be > 0
comm -23 \
  <(readelf --dyn-syms -W apk_extract/lib/armeabi-v7a/libdart_ffi.so \
      | awk '$7=="UND"{print $8}' | sort -u) \
  <(readelf --dyn-syms -W apk_extract/lib/armeabi-v7a/libc++_shared.so \
      | awk '{print $8}' | sort -u | sed 's/@.*//')   # must be empty

# --- symbol provenance (which NDK built which .so) --------------------
readelf -p .comment  apk_extract/lib/armeabi-v7a/libdart_ffi.so   # "Android (NDK rXX)" strings
strings -a libdart_ffi.so | grep -m1 "Android \(NDK\|clang\)"

# --- build + release (this fork) --------------------------------------
# .github/workflows/build-armeabiv7-glm.yml  → Build Android APK (ARMeabiv7)
#   Flutter 3.27.4 · Rust 1.85.0 · cargo-ndk 3.5.4 · NDK 27.3.13750724 (r27d)
#   artifact + GitHub release: appflowy-armeabi-v7a.apk + .sha256, tag = pubspec version
```
