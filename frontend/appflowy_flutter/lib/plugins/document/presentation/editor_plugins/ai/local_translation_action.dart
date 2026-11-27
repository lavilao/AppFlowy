import 'package:appflowy/ai/intelligence/local_translation_service.dart';
import 'package:appflowy/generated/flowy_svgs.g.dart';
import 'package:appflowy/generated/locale_keys.g.dart';
import 'package:appflowy/mobile/presentation/bottom_sheet/show_transition_bottom_sheet.dart';
import 'package:appflowy/plugins/document/presentation/editor_plugins/ai/local_intelligence_actions.dart';
import 'package:appflowy/workspace/presentation/widgets/dialogs.dart';
import 'package:appflowy_backend/log.dart';
import 'package:appflowy_editor/appflowy_editor.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:google_mlkit_translation/google_mlkit_translation.dart';

/// Translates the selection (or the whole note when nothing is selected) with
/// ML Kit's on-device translator and inserts the result below it.
///
/// The language list is the ML Kit set, and the models are downloaded the first
/// time a language is used, so the action needs no account and no network
/// afterwards.
Future<void> runLocalTranslationAction(
  BuildContext context,
  EditorState editorState,
) async {
  if (!LocalTranslationService.isSupported) {
    _toast(LocaleKeys.document_plugins_aiWriter_translateUnavailable.tr());
    return;
  }

  final text = _textToTranslate(editorState);
  if (text.trim().isEmpty) {
    _toast(LocaleKeys.document_plugins_aiWriter_nothingToTranslate.tr());
    return;
  }

  final target = await _pickTargetLanguage(context);
  if (target == null) {
    return;
  }

  try {
    final result = await LocalTranslationService.translate(
      text: text,
      target: target,
      onDownloading: (_) =>
          _toast(LocaleKeys.document_plugins_aiWriter_translateDownloading.tr()),
    );
    await insertParagraphsBelowSelection(editorState, result.text);
  } on Exception catch (error, stackTrace) {
    Log.error('Local translation failed: $error\n$stackTrace');
    _toast(LocaleKeys.document_plugins_aiWriter_translateFailed.tr());
  }
}

/// The same rule as the other on-device actions: a selection wins, otherwise
/// the whole note is translated.
String _textToTranslate(EditorState editorState) {
  final selection = editorState.selection?.normalized;
  if (selection != null && !selection.isCollapsed) {
    return editorState.getTextInSelection(selection).join('\n');
  }
  return editorState.document.root.children
      .map((node) => node.delta?.toPlainText() ?? '')
      .join('\n');
}

Future<TranslateLanguage?> _pickTargetLanguage(BuildContext context) {
  return showTransitionMobileBottomSheet<TranslateLanguage>(
    context,
    showDragHandle: true,
    showHeader: true,
    title: LocaleKeys.document_plugins_aiWriter_translateTarget.tr(),
    initialStop: 0.9,
    builder: (context) => const _LanguagePicker(),
  );
}

/// Searchable list of the languages ML Kit can translate into.
class _LanguagePicker extends StatefulWidget {
  const _LanguagePicker();

  @override
  State<_LanguagePicker> createState() => _LanguagePickerState();
}

class _LanguagePickerState extends State<_LanguagePicker> {
  final _controller = TextEditingController();
  String _query = '';

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final languages = LocalTranslationService.languages.where((language) {
      if (_query.isEmpty) {
        return true;
      }
      return LocalTranslationService.displayName(language)
          .toLowerCase()
          .contains(_query.toLowerCase());
    }).toList();

    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          child: TextField(
            controller: _controller,
            onChanged: (value) => setState(() => _query = value.trim()),
            decoration: InputDecoration(
              isDense: true,
              hintText:
                  LocaleKeys.document_plugins_aiWriter_translateSearch.tr(),
              prefixIcon: const FlowySvg(
                FlowySvgs.search_s,
                size: Size.square(16),
              ),
              border: OutlineInputBorder(
                borderRadius: BorderRadius.circular(8),
              ),
            ),
          ),
        ),
        Expanded(
          child: ListView.builder(
            itemCount: languages.length,
            itemBuilder: (context, index) {
              final language = languages[index];
              return ListTile(
                dense: true,
                title: Text(LocalTranslationService.displayName(language)),
                onTap: () => Navigator.of(context).pop(language),
              );
            },
          ),
        ),
      ],
    );
  }
}

void _toast(String message) => showToastNotification(message: message);