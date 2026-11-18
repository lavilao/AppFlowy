cmake_minimum_required(VERSION 3.10.0)

project(AppFlowy)

message(CONFIGURE_LOG "NDK PATH: ${ANDROID_NDK}")
message(CONFIGURE_LOG "Copying libc++_shared.so")

# Host tag for the NDK's toolchains/llvm/prebuilt/<host> directory.
if(WIN32)
    set(HOST_TAG "windows-x86_64")
elseif(APPLE)
    set(HOST_TAG "darwin-x86_64")
else()
    set(HOST_TAG "linux-x86_64")
endif()

set(SYSROOT "${ANDROID_NDK}/toolchains/llvm/prebuilt/${HOST_TAG}/sysroot")

# libc++_shared.so lives in the sysroot (usr/lib/<triple>) since NDK r21; the
# legacy sources/cxx-stl/llvm-libc++/libs/<abi>/ copies were REMOVED in NDK
# r27, so always copy from the sysroot.
#
# This runtime MUST come from the same NDK (ndkVersion in app/build.gradle)
# that builds/links the native code. libdart_ffi.so (the Rust backend, linked
# by cargo-ndk) resolves its 119 __ndk1 symbols against the libc++_shared.so
# packaged in this APK at dlopen() time. A mismatch — e.g. an NDK r24 runtime
# packaged next to an r27d-linked libdart_ffi.so — fails on launch with:
#   dlopen failed: cannot locate symbol
#   "_ZTTNSt6__ndk118basic_stringstreamIcNS_11char_traitsIcEENS_9allocatorIcEEEE"
# i.e. the black-screen crash on 32-bit devices before the first Flutter frame.
# This copy also overwrites the (same-version) libc++_shared.so that cargo-ndk
# drops into jniLibs, so the packaged runtime always matches ndkVersion.

# arm64-v8a
file(COPY
    ${SYSROOT}/usr/lib/aarch64-linux-android/libc++_shared.so
    DESTINATION ${CMAKE_CURRENT_SOURCE_DIR}/jniLibs/arm64-v8a
)

# armeabi-v7a
file(COPY
    ${SYSROOT}/usr/lib/arm-linux-androideabi/libc++_shared.so
    DESTINATION ${CMAKE_CURRENT_SOURCE_DIR}/jniLibs/armeabi-v7a
)

# x86_64
file(COPY
    ${SYSROOT}/usr/lib/x86_64-linux-android/libc++_shared.so
    DESTINATION ${CMAKE_CURRENT_SOURCE_DIR}/jniLibs/x86_64
)
