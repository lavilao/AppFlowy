import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:appflowy/ai/intelligence/cactus/cactus_native.dart';
import 'package:appflowy/ai/intelligence/cactus/cactus_weights.dart';
import 'package:appflowy_backend/log.dart';
import 'package:ffi/ffi.dart';
import 'package:flutter/foundation.dart';

/// Raised when the engine refuses a call. The message is the engine's own
/// `cactus_last_error`.
class CactusException implements Exception {
  const CactusException(this.message);

  final String message;

  @override
  String toString() => 'CactusException: $message';
}

const int _textBufferSize = 1 << 16;
const int _speechBufferSize = 1 << 18;
const int _visionBufferSize = 1 << 18;

CactusNative _openNative() =>
    CactusNative(DynamicLibrary.open(cactusLibraryName));

/// Loads [weightsPath] unless the engine already reports [capability].
///
/// The engine keeps process-global state, so this is safe from any isolate as
/// long as callers serialize (see [CactusEngine]).
void _ensureLoaded(CactusNative native, String weightsPath, int capability) {
  if (native.models() & capability != 0) {
    return;
  }
  final path = weightsPath.toNativeUtf8();
  try {
    final loaded = native.loadFile(path);
    if (loaded < 0) {
      throw CactusException(
        native.lastError() ?? 'the model could not be loaded ($loaded)',
      );
    }
  } finally {
    calloc.free(path);
  }
}

String _result(Pointer<Uint8> buffer, int code, CactusNative native) {
  if (code < 0) {
    throw CactusException(
      native.lastError() ?? 'the engine returned $code',
    );
  }
  return buffer.cast<Utf8>().toDartString();
}

/// One text turn with the toolset in [tools], run off the UI isolate.
String cactusCompleteIsolate({
  required String weightsPath,
  required String input,
  required String system,
  required String tools,
  required int maxNewTokens,
}) {
  final native = _openNative();
  _ensureLoaded(native, weightsPath, cactusTextCapability);

  final systemPtr = system.toNativeUtf8();
  final toolsPtr = tools.toNativeUtf8();
  final inputPtr = input.toNativeUtf8();
  final out = calloc<Uint8>(_textBufferSize);
  try {
    final prepared = native.init(systemPtr, toolsPtr);
    if (prepared < 0) {
      throw CactusException(
        native.lastError() ?? 'the toolset could not be prepared ($prepared)',
      );
    }
    final code = native.complete(inputPtr, maxNewTokens, out, _textBufferSize);
    return _result(out, code, native);
  } finally {
    calloc
      ..free(systemPtr)
      ..free(toolsPtr)
      ..free(inputPtr)
      ..free(out);
  }
}

/// Transcribes 16 kHz mono PCM, run off the UI isolate.
String cactusTranscribeIsolate({
  required String weightsPath,
  required Int16List samples,
  String? language,
}) {
  final native = _openNative();
  _ensureLoaded(native, weightsPath, cactusSpeechCapability);

  final pcm = calloc<Int16>(samples.length);
  final out = calloc<Uint8>(_speechBufferSize);
  final languagePtr =
      language == null ? nullptr : language.toNativeUtf8().cast<Utf8>();
  try {
    pcm.asTypedList(samples.length).setAll(0, samples);
    final code = native.transcribePcm16(
      pcm,
      samples.length,
      languagePtr,
      0,
      out,
      _speechBufferSize,
    );
    return _result(out, code, native);
  } finally {
    if (languagePtr != nullptr) {
      calloc.free(languagePtr);
    }
    calloc
      ..free(pcm)
      ..free(out);
  }
}

/// Reads a packed RGB image, run off the UI isolate.
String cactusDescribeIsolate({
  required String weightsPath,
  required Uint8List rgb,
  required int width,
  required int height,
  required String options,
}) {
  final native = _openNative();
  _ensureLoaded(native, weightsPath, cactusVisionCapability);

  final pixels = calloc<Uint8>(rgb.length);
  final out = calloc<Uint8>(_visionBufferSize);
  final optionsPtr = options.toNativeUtf8();
  try {
    pixels.asTypedList(rgb.length).setAll(0, rgb);
    final code = native.describeRgb(
      pixels,
      width,
      height,
      optionsPtr,
      nullptr,
      out,
      _visionBufferSize,
    );
    return _result(out, code, native);
  } finally {
    calloc
      ..free(pixels)
      ..free(optionsPtr)
      ..free(out);
  }
}

/// Embeds [text], run off the UI isolate. Returns the raw float buffer.
Float32List cactusEmbedIsolate({
  required String weightsPath,
  required String text,
}) {
  final native = _openNative();
  _ensureLoaded(native, weightsPath, cactusTextCapability);

  final textPtr = text.toNativeUtf8();
  try {
    final dimension = native.embed(textPtr, nullptr, 0);
    if (dimension <= 0) {
      throw CactusException(
        native.lastError() ?? 'no embedding was produced ($dimension)',
      );
    }
    final out = calloc<Float>(dimension);
    try {
      final code = native.embed(textPtr, out, dimension);
      if (code != dimension) {
        throw CactusException(
          native.lastError() ?? 'the embedding was truncated ($code)',
        );
      }
      return Float32List.fromList(out.asTypedList(dimension));
    } finally {
      calloc.free(out);
    }
  } finally {
    calloc.free(textPtr);
  }
}

/// One answer from the engine: the tool calls (or extraction) plus the
/// confidence and the reason the engine gives for them.
class CactusTurn {
  const CactusTurn(this.json);

  final Map<String, dynamic> json;

  bool get success => json['success'] == true;

  /// `call` when the model wants a tool, `respond` when the loop is done.
  String get type => json['type'] as String? ?? '';

  /// Calibrated 0..1 score, or null when the archive has no calibration head.
  double? get confidence => (json['confidence'] as num?)?.toDouble();

  List<Map<String, dynamic>> get calls =>
      _calls(json['function_calls']) ?? const [];

  /// Calls the engine withheld (below its 0.1 floor).
  List<Map<String, dynamic>> get suppressedCalls =>
      _calls(json['suppressed_calls']) ?? const [];

  /// The arguments of the first call named [name], or null when the model
  /// chose a different tool (or refused, which is an empty call list).
  Map<String, dynamic>? argumentsOf(String name) {
    for (final call in calls) {
      if (call['name'] == name && call['arguments'] is Map) {
        return (call['arguments'] as Map).cast<String, dynamic>();
      }
    }
    return null;
  }

  static List<Map<String, dynamic>>? _calls(Object? raw) {
    if (raw is! List) {
      return null;
    }
    return [
      for (final call in raw)
        if (call is Map) call.cast<String, dynamic>(),
    ];
  }
}

/// On-device intelligence through the Cactus engine.
///
/// The engine is a single shared library holding process-global state, so
/// every call goes through [_serialized] and the heavy ones run on a worker
/// isolate: a turn takes seconds on a phone and must not block the UI.
class CactusEngine {
  CactusEngine._();

  static final CactusEngine instance = CactusEngine._();

  Future<void> _queue = Future<void>.value();

  /// Android is the target the engine archives were added for.
  static bool get isSupported => !kIsWeb && Platform.isAndroid;

  /// Whether the model is bundled with this build and can run here.
  static Future<bool> isAvailable(CactusModel model) async {
    if (!isSupported) {
      return false;
    }
    return await CactusWeights.of(model) != null;
  }

  /// One text turn: [toolsJson] is the JSON array of functions the model may
  /// call, and the answer is returned as the engine wrote it.
  Future<CactusTurn> complete({
    required String input,
    required String toolsJson,
    String system = '',
    int maxNewTokens = 256,
  }) async {
    final weightsPath = await _pathFor(CactusModel.needle3);
    final answer = await _serialized(
      () => Isolate.run(
        () => cactusCompleteIsolate(
          weightsPath: weightsPath,
          input: input,
          system: system,
          tools: toolsJson,
          maxNewTokens: maxNewTokens,
        ),
      ),
    );
    return CactusTurn(_decode(answer));
  }

  /// Transcribes 16 kHz mono PCM and returns the plain text.
  Future<String> transcribe({
    required Int16List samples,
    String? language,
  }) async {
    if (samples.isEmpty) {
      return '';
    }
    final weightsPath = await _pathFor(CactusModel.whistle);
    final answer = await _serialized(
      () => Isolate.run(
        () => cactusTranscribeIsolate(
          weightsPath: weightsPath,
          samples: samples,
          language: language,
        ),
      ),
    );
    final decoded = _decode(answer);
    return (decoded['text'] as String? ?? '').trim();
  }

  /// Reads a packed RGB image. [options] selects what to look at, e.g.
  /// `["text"]` for the text reader or `["objects","text"]` for a
  /// description.
  Future<Map<String, dynamic>> describe({
    required Uint8List rgb,
    required int width,
    required int height,
    required String options,
  }) async {
    final weightsPath = await _pathFor(CactusModel.mirage);
    final answer = await _serialized(
      () => Isolate.run(
        () => cactusDescribeIsolate(
          weightsPath: weightsPath,
          rgb: rgb,
          width: width,
          height: height,
          options: options,
        ),
      ),
    );
    return _decode(answer);
  }

  /// The local embedding of [text], for semantic search and duplicate
  /// detection. Never leaves the device.
  Future<List<double>> embed(String text) async {
    final weightsPath = await _pathFor(CactusModel.needle3);
    return _serialized(
      () => Isolate.run(
        () => cactusEmbedIsolate(weightsPath: weightsPath, text: text),
      ),
    );
  }

  /// Clears the conversation while keeping the model in memory.
  Future<void> reset() => _serialized(() async {
        _openNative().reset();
      });

  Future<String> _pathFor(CactusModel model) async {
    final weight = await CactusWeights.of(model);
    if (weight == null) {
      throw CactusException('${model.id} is not bundled with this build');
    }
    return CactusWeights.materialize(weight);
  }

  Map<String, dynamic> _decode(String raw) {
    try {
      return (jsonDecode(raw) as Map).cast<String, dynamic>();
    } on FormatException catch (error) {
      Log.error('Cactus returned something that is not JSON: $error\n$raw');
      throw const CactusException('the engine returned an unexpected answer');
    }
  }

  /// Serializes engine access: the library state is shared by every isolate.
  Future<T> _serialized<T>(Future<T> Function() action) {
    final completer = Completer<void>();
    final previous = _queue;
    _queue = completer.future;
    return previous.then((_) => action()).whenComplete(completer.complete);
  }
}
