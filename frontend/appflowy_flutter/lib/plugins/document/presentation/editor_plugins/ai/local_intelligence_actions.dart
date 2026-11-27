import 'package:appflowy/ai/intelligence/cactus/cactus_engine.dart';
import 'package:appflowy/ai/intelligence/cactus/cactus_weights.dart';
import 'package:appflowy/ai/intelligence/local_intelligence_service.dart';
import 'package:appflowy/generated/locale_keys.g.dart';
import 'package:appflowy/plugins/document/presentation/editor_plugins/ai/cactus_tasks_action.dart';
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
    LocalIntelligenceAction.extractActionItems =>
      await _extractActionItems(text),
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
  // Needle 3 is the only one of the two that can carry a due date through, so
  // it goes first. It answers with a call only when it is sure; a refusal (or
  // an unusable shape) leaves the extraction below to do the work.
  if (await CactusEngine.isAvailable(CactusModel.needle3)) {
    try {
      final markdown = await extractActionItemsWithCactus(text);
      if (markdown != null) {
        return markdown;
      }
    } on Exception catch (error, stackTrace) {
      Log.error('Needle action items failed: $error\n$stackTrace');
    }
  }

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

/// Inserts [text] at [caret], or at the caret of the document when the caret
/// moved while [caret] was waiting (dictation).
///
/// The mobile toolbar rebuilds its overlay while it runs, so the selection it
/// captured when the action started is passed in explicitly.
Future<void> insertPlainText(
  EditorState editorState,
  Selection caret,
  String text,
) async {
  final current = editorState.selection;
  final target = (current != null && current.isCollapsed) ? current : caret;
  if (!target.isCollapsed) {
    return;
  }
  final node = editorState.getNodeAtPath(target.end.path);
  if (node == null) {
    return;
  }
  final transaction = editorState.transaction
    ..insertText(node, target.end.offset, text);
  await editorState.apply(transaction);
}

/// Inserts [text] as plain paragraphs below the selection (or at the end of
/// the note when nothing is selected).
///
/// Used by the translation action so the translated text is regular, editable
/// blocks of the document rather than a special widget.
Future<void> insertParagraphsBelowSelection(
  EditorState editorState,
  String text,
) async {
  final lines = text
      .split('\n')
      .map((line) => line.trim())
      .where((line) => line.isNotEmpty)
      .map((line) => paragraphNode(text: line))
      .toList();
  if (lines.isEmpty) {
    return;
  }

  final selection = editorState.selection?.normalized;
  final insertPath = selection != null
      ? selection.end.path.next
      : editorState.document.root.children.last.path.next;

  final transaction = editorState.transaction..insertNodes(insertPath, lines);
  await editorState.apply(transaction);
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
