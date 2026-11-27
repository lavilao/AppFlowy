import 'dart:convert';

import 'package:appflowy/ai/intelligence/cactus/cactus_engine.dart';

/// One tool, described the way the model reads it literally: "the action, word
/// for word from the note" keeps the tasks verbatim instead of paraphrased.
const _actionItemsTool = '''
[
  {
    "name": "save_action_items",
    "description": "Save the action items found in the note",
    "parameters": {
      "type": "object",
      "properties": {
        "items": {
          "type": "array",
          "items": {
            "type": "object",
            "properties": {
              "task": {
                "type": "string",
                "description": "the action, word for word from the note"
              },
              "due": {
                "type": "string",
                "description": "the day or time the note gives, as it is written"
              }
            },
            "required": ["task"]
          }
        }
      },
      "required": ["items"]
    }
  }
]
''';

/// Extracts to-dos with their due dates using the bundled Needle 3 model.
///
/// Unlike the extractive engine, the model can carry "by Friday" through as a
/// date. It only answers with a call when it is sure: when it withholds the
/// call, or answers in a shape this cannot use, the caller keeps the to-dos it
/// would have produced anyway, so a declined note never loses its checklist.
///
/// Returns null when there is nothing to insert.
Future<String?> extractActionItemsWithCactus(String text) async {
  final turn = await CactusEngine.instance.complete(
    input: text,
    toolsJson: _actionItemsTool,
    system: 'date: ${_today()}',
    maxNewTokens: 320,
  );
  if (!turn.success) {
    return null;
  }

  final items = turn.argumentsOf('save_action_items')?['items'];
  if (items is! List) {
    return null;
  }

  final lines = <String>[];
  for (final item in items) {
    if (item is! Map) {
      continue;
    }
    final task = (item['task'] as Object?)?.toString().trim() ?? '';
    if (task.isEmpty) {
      continue;
    }
    final due = (item['due'] as Object?)?.toString().trim() ?? '';
    lines.add(due.isEmpty ? '- [ ] $task' : '- [ ] $task ($due)');
  }
  return lines.isEmpty ? null : lines.join('\n');
}

/// The model resolves relative dates ("tomorrow", "Friday") against this, so
/// it has to be the device's own date.
String _today() {
  final now = DateTime.now();
  final weekday = const [
    'Mon',
    'Tue',
    'Wed',
    'Thu',
    'Fri',
    'Sat',
    'Sun',
  ][now.weekday - 1];
  final month = now.month.toString().padLeft(2, '0');
  final day = now.day.toString().padLeft(2, '0');
  final hour = now.hour.toString().padLeft(2, '0');
  final minute = now.minute.toString().padLeft(2, '0');
  return '${now.year}-$month-$day $weekday $hour:$minute';
}

/// Kept for callers that want to inspect the raw answer while debugging.
String debugTurnJson(CactusTurn turn) => jsonEncode(turn.json);
