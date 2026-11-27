import 'dart:typed_data';

import 'package:appflowy/ai/intelligence/cactus/cactus_engine.dart';
import 'package:appflowy/generated/locale_keys.g.dart';
import 'package:appflowy/plugins/document/presentation/editor_plugins/ai/local_intelligence_actions.dart';
import 'package:appflowy/workspace/presentation/widgets/dialogs.dart';
import 'package:appflowy_backend/log.dart';
import 'package:appflowy_editor/appflowy_editor.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:image/image.dart' as img;
import 'package:image_picker/image_picker.dart';

/// Images are downscaled before they are read: the model works on a bounded
/// picture and a modern phone photo is many times larger than it needs.
const int _maxScanEdge = 1280;

/// Reads the printed text of a picture and inserts it into the note.
///
/// The picture is decoded and read by the bundled Mirage model on the phone;
/// it is never uploaded. Mirage reads Latin script, which is what the action
/// tells the user when it finds nothing.
Future<void> runLocalScanAction(EditorState editorState) async {
  final picked = await ImagePicker().pickImage(source: ImageSource.gallery);
  if (picked == null) {
    return;
  }

  try {
    final decoded = img.decodeImage(await picked.readAsBytes());
    if (decoded == null) {
      _toast(LocaleKeys.document_plugins_aiWriter_scanUnreadable.tr());
      return;
    }

    final pixels = _rgbOf(decoded);
    final result = await CactusEngine.instance.describe(
      rgb: pixels.bytes,
      width: pixels.width,
      height: pixels.height,
      options: '{"sections":["text"]}',
    );

    final text = _textOf(result);
    if (text.isEmpty) {
      _toast(LocaleKeys.document_plugins_aiWriter_scanNoText.tr());
      return;
    }
    await insertParagraphsBelowSelection(editorState, text);
  } on Exception catch (error, stackTrace) {
    Log.error('Reading the picture failed: $error\n$stackTrace');
    _toast(LocaleKeys.document_plugins_aiWriter_scanFailed.tr());
  }
}

class _Rgb {
  const _Rgb(this.bytes, this.width, this.height);

  final Uint8List bytes;
  final int width;
  final int height;
}

_Rgb _rgbOf(img.Image image) {
  final longest = image.width > image.height ? image.width : image.height;
  // Scale both edges: resizing only the width would stretch the picture and
  // make the text reader's job harder.
  final scale = longest > _maxScanEdge ? _maxScanEdge / longest : 1.0;
  final resized = scale < 1.0
      ? img.copyResize(
          image,
          width: (image.width * scale).round(),
          height: (image.height * scale).round(),
        )
      : image;
  // Mirage takes packed 8-bit RGB, not the decoded image object.
  final rgb = resized.convert(numChannels: 3);
  return _Rgb(
    rgb.getBytes(order: img.ChannelOrder.rgb),
    rgb.width,
    rgb.height,
  );
}

/// Mirage answers with a JSON object whose shape depends on the requested
/// sections, so the text is collected structurally rather than by exact key.
String _textOf(Map<String, dynamic> result) {
  final lines = <String>[];
  for (final line in _strings(result)) {
    final trimmed = line.trim();
    if (trimmed.isEmpty || _sectionNames.contains(trimmed.toLowerCase())) {
      continue;
    }
    lines.add(trimmed);
  }
  return lines.join('\n');
}

const _sectionNames = {'text', 'objects', 'sections', 'result'};

/// Walks [node] for its strings, preferring a `text` key when there is one.
Iterable<String> _strings(Object? node) sync* {
  if (node is String) {
    yield node;
    return;
  }
  if (node is Map) {
    for (final key in const ['text', 'lines', 'content', 'value']) {
      if (node.containsKey(key)) {
        yield* _strings(node[key]);
        return;
      }
    }
    for (final value in node.values) {
      yield* _strings(value);
    }
    return;
  }
  if (node is List) {
    for (final value in node) {
      yield* _strings(value);
    }
  }
}

void _toast(String message) => showToastNotification(message: message);
