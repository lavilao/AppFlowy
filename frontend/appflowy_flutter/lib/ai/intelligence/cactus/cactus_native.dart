import 'dart:ffi';

import 'package:ffi/ffi.dart';

/// Capability bits reported by the engine (`cactus.h`).
const int cactusTextCapability = 1;
const int cactusSpeechCapability = 2;
const int cactusVisionCapability = 4;

/// Name of the shared library built from the Cactus engine
/// (see android/app/src/main/CMakeLists.txt).
const String cactusLibraryName = 'libflowy_cactus.so';

/// Raw bindings to the Cactus bridge.
///
/// Every function is looked up in the constructor so a missing symbol fails
/// loudly at load time instead of at the first call.
class CactusNative {
  CactusNative(DynamicLibrary library)
      : _release = library.lookupFunction<Void Function(), void Function()>(
          'flowy_cactus_release',
        ),
        _loadFile = library
            .lookupFunction<Int32 Function(Pointer<Utf8>), int Function(Pointer<Utf8>)>(
          'flowy_cactus_load_file',
        ),
        _models = library.lookupFunction<Int32 Function(), int Function()>(
          'flowy_cactus_models',
        ),
        _lastError =
            library.lookupFunction<Pointer<Utf8> Function(), Pointer<Utf8> Function()>(
          'flowy_cactus_last_error',
        ),
        _setThreads =
            library.lookupFunction<Void Function(Int32), void Function(int)>(
          'flowy_cactus_set_threads',
        ),
        _init = library.lookupFunction<
            Int32 Function(Pointer<Utf8>, Pointer<Utf8>),
            int Function(Pointer<Utf8>, Pointer<Utf8>)>(
          'flowy_cactus_init',
        ),
        _reset = library.lookupFunction<Void Function(), void Function()>(
          'flowy_cactus_reset',
        ),
        _complete = library.lookupFunction<
            Int32 Function(Pointer<Utf8>, Int32, Pointer<Uint8>, Int32),
            int Function(Pointer<Utf8>, int, Pointer<Uint8>, int)>(
          'flowy_cactus_complete',
        ),
        _transcribePcm16 = library.lookupFunction<
            Int32 Function(
              Pointer<Int16>,
              Int32,
              Pointer<Utf8>,
              Int32,
              Pointer<Uint8>,
              Int32,
            ),
            int Function(
              Pointer<Int16>,
              int,
              Pointer<Utf8>,
              int,
              Pointer<Uint8>,
              int,
            )>('flowy_cactus_transcribe_pcm16'),
        _describeRgb = library.lookupFunction<
            Int32 Function(
              Pointer<Uint8>,
              Int32,
              Int32,
              Pointer<Utf8>,
              Pointer<Utf8>,
              Pointer<Uint8>,
              Int32,
            ),
            int Function(
              Pointer<Uint8>,
              int,
              int,
              Pointer<Utf8>,
              Pointer<Utf8>,
              Pointer<Uint8>,
              int,
            )>('flowy_cactus_describe_rgb'),
        _embed = library.lookupFunction<
            Int32 Function(Pointer<Utf8>, Pointer<Float>, Int32),
            int Function(Pointer<Utf8>, Pointer<Float>, int)>(
          'flowy_cactus_embed',
        ),
        _version = library.lookupFunction<
            Pointer<Utf8> Function(),
            Pointer<Utf8> Function()>('flowy_cactus_engine_version');

  final void Function() _release;
  final int Function(Pointer<Utf8>) _loadFile;
  final int Function() _models;
  final Pointer<Utf8> Function() _lastError;
  final void Function(int) _setThreads;
  final int Function(Pointer<Utf8>, Pointer<Utf8>) _init;
  final void Function() _reset;
  final int Function(Pointer<Utf8>, int, Pointer<Uint8>, int) _complete;
  final int Function(Pointer<Int16>, int, Pointer<Utf8>, int, Pointer<Uint8>, int)
      _transcribePcm16;
  final int Function(Pointer<Uint8>, int, int, Pointer<Utf8>, Pointer<Utf8>,
      Pointer<Uint8>, int) _describeRgb;
  final int Function(Pointer<Utf8>, Pointer<Float>, int) _embed;
  final Pointer<Utf8> Function() _version;

  void release() => _release();

  int loadFile(Pointer<Utf8> path) => _loadFile(path);

  int models() => _models();

  String? lastError() => _lastError().toDartString();

  void setThreads(int threads) => _setThreads(threads);

  int init(Pointer<Utf8> systemPrompt, Pointer<Utf8> toolsJson) =>
      _init(systemPrompt, toolsJson);

  void reset() => _reset();

  int complete(
    Pointer<Utf8> input,
    int maxNewTokens,
    Pointer<Uint8> out,
    int capacity,
  ) =>
      _complete(input, maxNewTokens, out, capacity);

  int transcribePcm16(
    Pointer<Int16> pcm,
    int samples,
    Pointer<Utf8> language,
    int wordTimestamps,
    Pointer<Uint8> out,
    int capacity,
  ) =>
      _transcribePcm16(pcm, samples, language, wordTimestamps, out, capacity);

  int describeRgb(
    Pointer<Uint8> rgb,
    int width,
    int height,
    Pointer<Utf8> options,
    Pointer<Utf8> language,
    Pointer<Uint8> out,
    int capacity,
  ) =>
      _describeRgb(rgb, width, height, options, language, out, capacity);

  int embed(Pointer<Utf8> text, Pointer<Float> out, int capacity) =>
      _embed(text, out, capacity);

  String version() => _version().toDartString();
}
