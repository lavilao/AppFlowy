import 'dart:io';

import 'package:appflowy_backend/log.dart';
import 'package:flutter/foundation.dart';
import 'package:google_mlkit_language_id/google_mlkit_language_id.dart';
import 'package:google_mlkit_translation/google_mlkit_translation.dart';

/// A translation produced on the device by ML Kit.
class LocalTranslation {
  const LocalTranslation({
    required this.text,
    required this.source,
    required this.target,
  });

  final String text;
  final TranslateLanguage source;
  final TranslateLanguage target;

  /// e.g. "French", for the confirmations.
  String get targetName => LocalTranslationService.displayName(target);
}

/// On-device translation through ML Kit.
///
/// Nothing leaves the phone: the language models are downloaded once (Wi-Fi is
/// not required, they are small) and the text is translated by the models that
/// ship with Google Play services. [isSupported] is false on desktop and web,
/// where the plugin has no implementation.
class LocalTranslationService {
  /// ML Kit translates between 50+ languages. The models are per language, so
  /// the list is the full ML Kit set rather than a curated subset.
  static List<TranslateLanguage> get languages => TranslateLanguage.values;

  static bool get isSupported =>
      !kIsWeb && (Platform.isAndroid || Platform.isIOS);

  /// "haitianCreole" -> "Haitian Creole".
  static String displayName(TranslateLanguage language) {
    final name = language.name;
    final buffer = StringBuffer();
    for (var i = 0; i < name.length; i++) {
      final char = name[i];
      if (i > 0 && char.toUpperCase() == char && char.toLowerCase() != char) {
        buffer.write(' ');
      }
      buffer.write(i == 0 ? char.toUpperCase() : char);
    }
    return buffer.toString();
  }

  /// Detects the language of [text].
  ///
  /// ML Kit's translator requires an explicit source language, so this runs
  /// first. Returns null when the identifier is not confident enough.
  static Future<TranslateLanguage?> detect(String text) async {
    final identifier = LanguageIdentifier(confidenceThreshold: 0.3);
    try {
      final possible = await identifier.identifyPossibleLanguages(text);
      if (possible.isEmpty) {
        return null;
      }
      // ML Kit reports a BCP-47 tag that may carry a region ("pt-BR"); the
      // translator's own codes are the bare language.
      final tag = possible.first.languageTag;
      return BCP47Code.fromRawValue(tag) ??
          BCP47Code.fromRawValue(tag.split('-').first);
    } on Exception catch (error, stackTrace) {
      Log.error('Local language detection failed: $error\n$stackTrace');
      return null;
    } finally {
      identifier.close();
    }
  }

  /// Translates [text] into [target], downloading the models when needed.
  ///
  /// Throws when the models cannot be downloaded or translation fails; the
  /// caller turns that into a single message so the editor stays untouched.
  static Future<LocalTranslation> translate({
    required String text,
    required TranslateLanguage target,
    void Function(String languageCode)? onDownloading,
  }) async {
    final detected = await detect(text);
    // A note already written in the target language is translated from English
    // rather than "translated" into itself, which ML Kit rejects.
    final source = detected == null || detected == target
        ? (target == TranslateLanguage.english
            ? TranslateLanguage.french
            : TranslateLanguage.english)
        : detected;

    if (!await _isModelDownloaded(source.bcpCode)) {
      onDownloading?.call(source.bcpCode);
      await _download(source.bcpCode);
    }
    if (!await _isModelDownloaded(target.bcpCode)) {
      onDownloading?.call(target.bcpCode);
      await _download(target.bcpCode);
    }

    final translator = OnDeviceTranslator(
      sourceLanguage: source,
      targetLanguage: target,
    );
    try {
      final translated = <String>[];
      for (final chunk in _chunks(text)) {
        translated.add(await translator.translateText(chunk));
      }
      return LocalTranslation(
        text: translated.join('\n'),
        source: source,
        target: target,
      );
    } finally {
      await translator.close();
    }
  }

  static Future<bool> _isModelDownloaded(String bcpCode) =>
      OnDeviceTranslatorModelManager().isModelDownloaded(bcpCode);

  static Future<void> _download(String bcpCode) async {
    // The language models are a few megabytes; downloading over mobile data is
    // fine and asking for Wi-Fi would leave the button dead on the phone.
    final downloaded =
        await OnDeviceTranslatorModelManager().downloadModel(
      bcpCode,
      isWifiRequired: false,
    );
    if (!downloaded) {
      throw StateError('language model $bcpCode is unavailable');
    }
  }

  /// Splits [text] on paragraph and sentence boundaries.
  ///
  /// ML Kit translates a whole string, but a very long note makes the call
  /// slow and the models work best with bounded input.
  static List<String> _chunks(String text, {int maxLength = 900}) {
    final chunks = <String>[];
    for (final paragraph in text.split('\n')) {
      if (paragraph.length <= maxLength) {
        if (paragraph.trim().isNotEmpty) {
          chunks.add(paragraph);
        }
        continue;
      }
      final buffer = StringBuffer();
      for (final sentence in paragraph.split(RegExp(r'(?<=[.!?])\s+'))) {
        if (buffer.length + sentence.length > maxLength && buffer.isNotEmpty) {
          chunks.add(buffer.toString());
          buffer.clear();
        }
        if (buffer.isNotEmpty) {
          buffer.write(' ');
        }
        buffer.write(sentence);
      }
      if (buffer.toString().trim().isNotEmpty) {
        chunks.add(buffer.toString());
      }
    }
    return chunks;
  }
}