import 'package:appflowy/generated/flowy_svgs.g.dart';
import 'package:appflowy/plugins/document/presentation/editor_plugins/ai/local_intelligence_actions.dart';
import 'package:appflowy/plugins/document/presentation/editor_plugins/ai/local_voice_input.dart';
import 'package:appflowy/plugins/document/presentation/editor_plugins/mobile_toolbar_v3/appflowy_mobile_toolbar_item.dart';
import 'package:flutter/widgets.dart';

/// On-device intelligence actions for the mobile toolbar.
///
/// They run offline on the selected text (or on the whole document when the
/// selection is collapsed) and insert regular blocks, so they need no account,
/// no model and no extra menu.
final localSummarizeToolbarItem = AppFlowyMobileToolbarItem(
  itemBuilder: (context, editorState, _, __, onAction) {
    return AppFlowyMobileToolbarIconItem(
      editorState: editorState,
      icon: FlowySvgs.ai_summarize_s,
      onTap: () => runLocalIntelligenceAction(
        editorState,
        LocalIntelligenceAction.summarize,
      ),
    );
  },
);

final localExtractActionItemsToolbarItem = AppFlowyMobileToolbarItem(
  itemBuilder: (context, editorState, _, __, onAction) {
    return AppFlowyMobileToolbarIconItem(
      editorState: editorState,
      icon: FlowySvgs.checklist_s,
      onTap: () => runLocalIntelligenceAction(
        editorState,
        LocalIntelligenceAction.extractActionItems,
      ),
    );
  },
);

final _voiceInput = LocalVoiceInput();

/// Dictation: writes what the device's speech recognizer hears at the caret.
///
/// It stays highlighted while the recognizer listens, the way a formatting
/// toggle does, and turns off by itself when the OS ends the session.
final localVoiceToolbarItem = AppFlowyMobileToolbarItem(
  itemBuilder: (context, editorState, _, __, ___) {
    return ValueListenableBuilder<bool>(
      valueListenable: _voiceInput.listening,
      builder: (context, isListening, __) {
        return AppFlowyMobileToolbarIconItem(
          editorState: editorState,
          icon: FlowySvgs.microphone_s,
          isSelected: () => isListening,
          onTap: () => isListening
              ? _voiceInput.stop()
              : _voiceInput.start(editorState),
        );
      },
    );
  },
);