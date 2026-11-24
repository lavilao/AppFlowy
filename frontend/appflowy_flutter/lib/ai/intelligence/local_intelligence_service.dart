import 'package:appflowy_backend/dispatch/dispatch.dart';
import 'package:appflowy_backend/protobuf/flowy-error/errors.pb.dart';
import 'package:appflowy_backend/protobuf/flowy-intelligence/protobuf.dart';
import 'package:appflowy_result/appflowy_result.dart';
import 'package:fixnum/fixnum.dart';

/// Typed access to AppFlowy's on-device intelligence engine.
///
/// Every call is served by the Rust `flowy-intelligence` plugin: deterministic,
/// offline and free of charge. Nothing here contacts the cloud, so these
/// features work for signed-out users and in airplane mode.
class LocalIntelligenceService {
  const LocalIntelligenceService._();

  /// Extractive summary of [text] plus an estimated reading time.
  static Future<FlowyResult<SummaryPB, FlowyError>> summarize({
    required String text,
    String title = '',
    int maxBullets = 3,
  }) {
    final request = SummarizeRequestPB()
      ..title = title
      ..text = text
      ..maxBullets = Int64(maxBullets);
    return IntelligenceEventSummarize(request).send();
  }

  /// Pulls action items out of note prose, checkboxes and voice transcripts.
  static Future<FlowyResult<RepeatedActionItemPB, FlowyError>> extractActionItems({
    required String text,
    String title = '',
    List<String> transcripts = const [],
    int maxItems = 5,
  }) {
    final request = ActionItemsRequestPB()
      ..title = title
      ..body = text
      ..maxItems = Int64(maxItems)
      ..transcripts.addAll(transcripts);
    return IntelligenceEventExtractActionItems(request).send();
  }

  /// Suggests a title for an untitled document. Returns an empty string when
  /// there is nothing meaningful to work with.
  static Future<FlowyResult<TitleSuggestionPB, FlowyError>> suggestTitle(
    String text,
  ) {
    return IntelligenceEventSuggestTitle(TitleRequestPB()..text = text).send();
  }

  /// People, places, organizations, projects, topics, links and dates found in
  /// [text], with byte offsets for inline highlighting.
  static Future<FlowyResult<RepeatedEntityPB, FlowyError>> extractEntities({
    required String text,
    String title = '',
    int limit = 10,
  }) {
    final request = ExtractEntitiesRequestPB()
      ..title = title
      ..text = text
      ..limit = Int64(limit);
    return IntelligenceEventExtractEntities(request).send();
  }

  /// The most connected entities across [documents]; the seed of a local
  /// knowledge graph.
  static Future<FlowyResult<RepeatedEntityHubPB, FlowyError>> entityHubs({
    required List<IntelligenceDocumentPB> documents,
    int limit = 12,
  }) {
    final request = GraphRequestPB()
      ..limit = Int64(limit)
      ..documents.addAll(documents);
    return IntelligenceEventEntityHubs(request).send();
  }

  /// Writing activity: streaks, notes this week, top tags and recurring topics.
  static Future<FlowyResult<WritingStatsPB, FlowyError>> writingStats(
    List<IntelligenceDocumentPB> documents,
  ) {
    final request = InsightsRequestPB()..documents.addAll(documents);
    return IntelligenceEventWritingStats(request).send();
  }

  /// BM25 + title/tag/phrase aware search that explains each hit.
  static Future<FlowyResult<RepeatedSearchHitPB, FlowyError>> smartSearch({
    required String query,
    required List<IntelligenceDocumentPB> documents,
    int limit = 10,
  }) {
    final request = SmartSearchRequestPB()
      ..query = query
      ..limit = Int64(limit)
      ..documents.addAll(documents);
    return IntelligenceEventSmartSearch(request).send();
  }

  /// Documents that talk about the same things as [document].
  static Future<FlowyResult<RepeatedSearchHitPB, FlowyError>> relatedDocuments({
    required IntelligenceDocumentPB document,
    required List<IntelligenceDocumentPB> documents,
    int limit = 5,
  }) {
    final request = RelatedDocumentsRequestPB()
      ..document = document
      ..limit = Int64(limit)
      ..documents.addAll(documents);
    return IntelligenceEventRelatedDocuments(request).send();
  }
}

/// Small builder so callers do not have to repeat the protobuf boilerplate.
IntelligenceDocumentPB intelligenceDocument({
  required String id,
  required String title,
  String body = '',
  List<String> tags = const [],
  int updatedAt = 0,
}) {
  return IntelligenceDocumentPB()
    ..id = id
    ..title = title
    ..body = body
    ..updatedAt = Int64(updatedAt)
    ..tags.addAll(tags);
}
