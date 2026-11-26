import 'dart:async';

import 'package:appflowy/generated/locale_keys.g.dart';
import 'package:appflowy/plugins/document/presentation/editor_plugins/ai/local_intelligence_actions.dart';
import 'package:appflowy/workspace/presentation/widgets/dialogs.dart';
import 'package:appflowy_backend/log.dart';
import 'package:appflowy_editor/appflowy_editor.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/foundation.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:speech_to_text/speech_recognition_error.dart';
import 'package:speech_to_text/speech_recognition_result.dart';
import 'package:speech_to_text/speech_to_text.dart';

/// How long dictation may go without a spoken pause before it ends by itself.
///
/// The recognizer also ends a session when the OS does (on Android, after a few
/// seconds of silence), which is why the button shows that it is listening.
const _dictationPause = Duration(seconds: 6);

/// Dictates text into the document with the speech recognizer of the device.
///
/// The recognizer belongs to the OS (on Android, Google's `SpeechRecognizer`),
/// so nothing is bundled with the app. Whether it runs on the device is up to
/// that service and the installed languages, which is what the permission
/// dialog says: AppFlowy itself never uploads the audio.
///
/// A session ends when the user taps the microphone again, when the recognizer
/// delivers its final result, or when the OS stops it.
class LocalVoiceInput {
  final SpeechToText _speech = SpeechToText();

  /// Whether the recognizer is capturing, for the toolbar highlight.
  final ValueNotifier<bool> listening = ValueNotifier(false);

  EditorState? _editorState;
  Selection? _caret;
  bool _isStarting = false;
  bool _written = false;
  String _transcript = '';

  /// Dictation is wired up for Android. The Apple builds need their own usage
  /// descriptions before the same entry point can be shown there, so the
  /// toolbar only adds the microphone where it can actually listen.
  static bool get isSupported =>
      !kIsWeb && defaultTargetPlatform == TargetPlatform.android;

  /// Starts listening and writes the transcript at the caret of [editorState].
  ///
  /// The caret is captured here on purpose: the overlay holding this action is
  /// rebuilt while the user talks, and dictation should not move it around.
  Future<void> start(EditorState editorState) async {
    if (_isStarting || _speech.isListening) {
      return;
    }
    _isStarting = true;
    try {
      if (!await _ensureMicrophone()) {
        _showPermission();
        return;
      }

      final selection = editorState.selection?.normalized;
      if (selection == null || !selection.isCollapsed) {
        _showCaret();
        return;
      }

      if (!await _speech.initialize(
        onStatus: _onStatus,
        onError: _onError,
        finalTimeout: const Duration(seconds: 2),
      )) {
        _showUnavailable();
        return;
      }

      _editorState = editorState;
      _caret = selection;
      _transcript = '';
      _written = false;
      await _speech.listen(
        onResult: _onResult,
        listenOptions: SpeechListenOptions(
          pauseFor: _dictationPause,
          listenFor: const Duration(minutes: 5),
          cancelOnError: true,
          partialResults: true,
        ),
      );
      if (!_speech.isListening) {
        // The recognizer never started: busy engine, no recognition service,
        // or a language it does not have.
        _reset();
        _showUnavailable();
        return;
      }
      _setListening(true);
    } on Exception catch (error, stackTrace) {
      Log.error('Local voice input failed to start: $error\n$stackTrace');
      _reset();
      _showUnavailable();
    } finally {
      _isStarting = false;
    }
  }

  Future<void> stop() async {
    if (!_speech.isListening) {
      return;
    }
    // Asks for the final result, which is the text that gets written.
    await _speech.stop();
  }

  void _onResult(SpeechRecognitionResult result) {
    _transcript = result.recognizedWords;
    if (!result.finalResult) {
      return;
    }
    // The session is over: write the words now, exactly once. [stop] only asks
    // the recognizer to deliver this result.
    unawaited(_commit());
  }

  void _onStatus(String status) {
    if (status == SpeechToText.listeningStatus) {
      _setListening(true);
      return;
    }
    _setListening(false);
    if (status == SpeechToText.doneStatus) {
      // The session is over and the final result has been delivered (or the
      // engine timed out waiting for it): write what was heard.
      unawaited(_commit());
    }
  }

  void _onError(SpeechRecognitionError error) {
    if (error.permanent) {
      Log.error('Local voice input error: ${error.errorMsg}');
      _reset();
      _showUnavailable();
    }
  }

  /// Writes the transcript at the caret captured in [start], once.
  Future<void> _commit() async {
    final editorState = _editorState;
    final caret = _caret;
    if (editorState == null || caret == null || _written) {
      _reset();
      return;
    }
    _written = true;

    final text = _transcript.trim();
    if (text.isEmpty) {
      _reset();
      _showNothingHeard();
      return;
    }
    await insertPlainText(editorState, caret, text);
    _reset();
  }

  void _reset() {
    _editorState = null;
    _caret = null;
    _transcript = '';
    _written = false;
    _setListening(false);
  }

  void _setListening(bool value) {
    if (listening.value != value) {
      listening.value = value;
    }
  }

  static Future<bool> _ensureMicrophone() async {
    final status = await Permission.microphone.status;
    if (status.isGranted) {
      return true;
    }
    if (status.isPermanentlyDenied) {
      return false;
    }
    final requested = await Permission.microphone.request();
    return requested.isGranted;
  }

  static void _showPermission() => _show(
        LocaleKeys.document_plugins_aiWriter_voiceInputPermission.tr(),
      );

  static void _showCaret() => _show(
        LocaleKeys.document_plugins_aiWriter_voiceInputCaret.tr(),
      );

  static void _showUnavailable() => _show(
        LocaleKeys.document_plugins_aiWriter_voiceInputUnavailable.tr(),
      );

  static void _showNothingHeard() => _show(
        LocaleKeys.document_plugins_aiWriter_voiceInputNothingHeard.tr(),
      );

  static void _show(String message) => showToastNotification(message: message);
}
