import 'package:appflowy/ai/intelligence/local_intelligence_service.dart';
import 'package:appflowy/generated/locale_keys.g.dart';
import 'package:appflowy/shared/markdown_to_document.dart';
import 'package:appflowy/workspace/presentation/widgets/dialogs.dart';
import 'package:appflowy_backend/log.dart';
import 'package:appflowy_backend/protobuf/flowy-intelligence/protobuf.dart';
import 'package:appflowy_editor/appflowy_editor.dart';
import 'package:easy_localization/easy_localization.dart';

/// Smart editor actions that run entirely on the device.
///
/// They intentionally reuse the document's own words (extractive summary,
/// extracted to-dos) and insert regular blocks, so the result is editable,
/// syncable and looks like something the user wrote.
enum LocalIntelligenceAction {
  summarize,
  extractActionItems,
}

Future<void> runLocalIntelligenceAction(
  EditorState editorState,
  LocalIntelligenceAction action,
) async {
  final selection = editorState.selection?.normalized;
  final text = _collectText(editorState, selection);
  if (text.trim().isEmpty) {
    _showNothingToAnalyze();
    return;
  }

  final markdown = switch (action) {
    LocalIntelligenceAction.summarize => await _summarize(text),
    LocalIntelligenceAction.extractActionItems => await _extractActionItems(text),
  };

  if (markdown == null || markdown.trim().isEmpty) {
    _showNothingToAnalyze();
    return;
  }

  await _insertMarkdown(editorState, markdown);
}

String _collectText(EditorState editorState, Selection? selection) {
  if (selection != null && !selection.isCollapsed) {
    return editorState.getTextInSelection(selection).join('\n');
  }
  return editorState.document.root.children
      .map((node) => node.delta?.toPlainText() ?? '')
      .join('\n');
}

Future<String?> _summarize(String text) async {
  final result = await LocalIntelligenceService.summarize(text: text);
  return result.fold(
    (summary) => summary.bullets.isEmpty
        ? null
        : summary.bullets.map((bullet) => '- $bullet').join('\n'),
    (error) {
      Log.error('Local summarize failed: $error');
      return null;
    },
  );
}

Future<String?> _extractActionItems(String text) async {
  final result = await LocalIntelligenceService.extractActionItems(text: text);
  return result.fold(
    (response) => _actionItemsMarkdown(response.items),
    (error) {
      Log.error('Local action item extraction failed: $error');
      return null;
    },
  );
}

String? _actionItemsMarkdown(List<ActionItemPB> items) {
  if (items.isEmpty) {
    return null;
  }
  return items.map((item) => '- [ ] ${item.title}').join('\n');
}

Future<void> _insertMarkdown(EditorState editorState, String markdown) async {
  final nodes = customMarkdownToDocument(markdown).root.children;
  if (nodes.isEmpty) {
    return;
  }

  final selection = editorState.selection?.normalized;
  final insertPath = selection != null
      ? selection.end.path.next
      : editorState.document.root.children.last.path.next;

  final transaction = editorState.transaction..insertNodes(insertPath, nodes);
  await editorState.apply(transaction);
}

void _showNothingToAnalyze() {
  showToastNotification(
    message: LocaleKeys.document_plugins_aiWriter_nothingToAnalyze.tr(),
  );
}
