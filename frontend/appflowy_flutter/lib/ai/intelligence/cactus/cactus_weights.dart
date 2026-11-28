import 'dart:convert';
import 'dart:io';

import 'package:appflowy_backend/log.dart';
import 'package:flutter/services.dart' show rootBundle;
import 'package:path_provider/path_provider.dart';

/// A Cactus model archive that ships with the app.
///
/// The three models run on one engine, one per capability: [text] calls tools
/// and extracts structured data, [speech] transcribes, [vision] reads images.
enum CactusModel {
  needle3(id: 'needle3'),
  whistle(id: 'whistle'),
  mirage(id: 'mirage');

  const CactusModel({required this.id});

  final String id;

  static CactusModel? fromId(String id) {
    for (final model in CactusModel.values) {
      if (model.id == id) {
        return model;
      }
    }
    return null;
  }
}

class CactusWeight {
  const CactusWeight({
    required this.model,
    required this.asset,
    required this.bytes,
    required this.sha256,
  });

  final CactusModel model;
  final String asset;
  final int bytes;
  final String sha256;
}

/// The models the build actually bundled.
///
/// `assets/cactus/manifest.json` is written by `scripts/fetch_cactus.sh`
/// together with the archives, so its presence means the weights are there. A
/// build without them simply offers no on-device features.
class CactusWeights {
  CactusWeights._(this.weights);

  final Map<CactusModel, CactusWeight> weights;

  static CactusWeights? _instance;
  static bool _loaded = false;

  static const _manifestAsset = 'assets/cactus/manifest.json';

  /// File names of the extracted archives, so they are read into native memory
  /// once and then memory-mapped from disk on every later load.
  static Future<CactusWeights?> load() async {
    if (_loaded) {
      return _instance;
    }
    _loaded = true;
    try {
      final raw = await rootBundle.loadString(_manifestAsset);
      final decoded = jsonDecode(raw) as Map<String, dynamic>;
      final models = decoded['models'] as Map<String, dynamic>;
      final weights = <CactusModel, CactusWeight>{};
      for (final entry in models.entries) {
        final model = CactusModel.fromId(entry.key);
        if (model == null) {
          continue;
        }
        final info = entry.value as Map<String, dynamic>;
        weights[model] = CactusWeight(
          model: model,
          asset: info['asset'] as String,
          bytes: info['bytes'] as int,
          sha256: info['sha256'] as String,
        );
      }
      _instance = CactusWeights._(weights);
    } on Exception catch (error) {
      Log.info('Cactus weights are not bundled: $error');
      _instance = null;
    }
    return _instance;
  }

  static Future<CactusWeight?> of(CactusModel model) async {
    final weights = await load();
    return weights?.weights[model];
  }

  /// Copies the packed asset to a real file the engine can map.
  ///
  /// Flutter assets live inside the APK, and `cactus_load` needs bytes that
  /// stay valid for the lifetime of the model, so the archive is unpacked into
  /// the app's support directory once.
  static Future<String> materialize(CactusWeight weight) async {
    final directory = Directory(
      '${(await getApplicationSupportDirectory()).path}/cactus',
    );
    if (!directory.existsSync()) {
      directory.createSync(recursive: true);
    }
    final file = File('${directory.path}/${weight.model.id}.cact');
    if (file.existsSync() && file.lengthSync() == weight.bytes) {
      return file.path;
    }

    final data = await rootBundle.load(weight.asset);
    await file.writeAsBytes(
      data.buffer.asUint8List(data.offsetInBytes, data.lengthInBytes),
      flush: true,
    );
    return file.path;
  }
}
