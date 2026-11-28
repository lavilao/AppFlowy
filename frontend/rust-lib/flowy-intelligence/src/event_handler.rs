use crate::entities::*;
use flowy_error::FlowyError;
use flowy_intelligence_core::entities::{self as engine_entities, DocEntities};
use flowy_intelligence_core::insights;
use flowy_intelligence_core::search;
use flowy_intelligence_core::summary;
use flowy_intelligence_core::title;
use lib_dispatch::prelude::{AFPluginData, DataResult, data_result_ok};

const MAX_BULLETS: i64 = 10;
const MAX_ITEMS: i64 = 20;
const DEFAULT_LIMIT: i64 = 10;

fn clamp_limit(value: Option<i64>, default: i64, maximum: i64) -> usize {
  value.unwrap_or(default).clamp(1, maximum) as usize
}

#[tracing::instrument(level = "debug", skip_all, err)]
pub(crate) async fn summarize_handler(
  data: AFPluginData<SummarizeRequestPB>,
) -> DataResult<SummaryPB, FlowyError> {
  let request = data.into_inner();
  let max_bullets = clamp_limit(request.max_bullets, 3, MAX_BULLETS);
  let result = summary::summarize(&request.title, &request.text, max_bullets);
  data_result_ok(SummaryPB {
    bullets: result.bullets,
    reading_minutes: result.reading_minutes as i64,
  })
}

#[tracing::instrument(level = "debug", skip_all, err)]
pub(crate) async fn extract_action_items_handler(
  data: AFPluginData<ActionItemsRequestPB>,
) -> DataResult<RepeatedActionItemPB, FlowyError> {
  let request = data.into_inner();
  let max_items = clamp_limit(request.max_items, 5, MAX_ITEMS);
  let items = flowy_intelligence_core::actions::extract_action_items(
    &request.title,
    &request.body,
    &request.transcripts,
    max_items,
  )
  .into_iter()
  .map(|item| ActionItemPB {
    title: item.title,
    due_in_minutes: item.due_in_minutes,
    source: item.source,
  })
  .collect();

  data_result_ok(RepeatedActionItemPB { items })
}

#[tracing::instrument(level = "debug", skip_all, err)]
pub(crate) async fn suggest_title_handler(
  data: AFPluginData<TitleRequestPB>,
) -> DataResult<TitleSuggestionPB, FlowyError> {
  let request = data.into_inner();
  let suggested = title::suggest_title(&request.text).unwrap_or_default();
  data_result_ok(TitleSuggestionPB { title: suggested })
}

#[tracing::instrument(level = "debug", skip_all, err)]
pub(crate) async fn extract_entities_handler(
  data: AFPluginData<ExtractEntitiesRequestPB>,
) -> DataResult<RepeatedEntityPB, FlowyError> {
  let request = data.into_inner();
  let limit = clamp_limit(request.limit, DEFAULT_LIMIT, 50);
  // Offsets are reported against `text` only, so callers can highlight them
  // directly in the editor. The title is passed separately for context.
  let documents = vec![DocEntities {
    document_id: String::new(),
    title: request.title,
    mentions: engine_entities::extract_mentions(&request.text),
  }];

  let items = engine_entities::top_entities(&documents, limit)
    .into_iter()
    .map(|node| {
      let (kind, display_name, offset_start, offset_end, confidence) = documents[0]
        .mentions
        .iter()
        .find(|mention| mention.raw == node.label)
        .map(|mention| {
          (
            mention.kind,
            mention.raw.clone(),
            mention.start as i64,
            mention.end as i64,
            mention.confidence,
          )
        })
        .unwrap_or((
          match node.kind {
            engine_entities::GraphNodeKind::Entity(kind) => kind,
            engine_entities::GraphNodeKind::Document => engine_entities::EntityKind::Topic,
          },
          node.label.clone(),
          0,
          0,
          0.0,
        ));

      EntityPB {
        display_name,
        kind: kind.into(),
        mentions: node.mentions as i64,
        confidence_percent: (confidence * 100.0).round() as i64,
        offset_start,
        offset_end,
      }
    })
    .collect();

  data_result_ok(RepeatedEntityPB { items })
}

#[tracing::instrument(level = "debug", skip_all, err)]
pub(crate) async fn entity_hubs_handler(
  data: AFPluginData<GraphRequestPB>,
) -> DataResult<RepeatedEntityHubPB, FlowyError> {
  let request = data.into_inner();
  let limit = clamp_limit(request.limit, 12, 100);

  let documents: Vec<DocEntities> = request
    .documents
    .into_iter()
    .map(|document| {
      let text = format!("{} {}", document.title, document.body);
      DocEntities {
        document_id: document.id,
        title: document.title,
        mentions: engine_entities::extract_mentions(&text),
      }
    })
    .collect();

  let graph = engine_entities::build_graph(&documents);
  let mut connections: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
  for edge in &graph.edges {
    *connections.entry(edge.source.clone()).or_insert(0) += edge.weight as i64;
    *connections.entry(edge.target.clone()).or_insert(0) += edge.weight as i64;
  }

  let items = engine_entities::hubs(&graph, limit)
    .into_iter()
    .map(|node| {
      let kind = match node.kind {
        engine_entities::GraphNodeKind::Entity(kind) => kind,
        engine_entities::GraphNodeKind::Document => engine_entities::EntityKind::Topic,
      };
      EntityHubPB {
        id: node.id.clone(),
        label: node.label,
        kind: kind.into(),
        mentions: node.mentions as i64,
        connections: connections.get(&node.id).copied().unwrap_or(0),
      }
    })
    .collect();

  data_result_ok(RepeatedEntityHubPB { items })
}

#[tracing::instrument(level = "debug", skip_all, err)]
pub(crate) async fn writing_stats_handler(
  data: AFPluginData<InsightsRequestPB>,
) -> DataResult<WritingStatsPB, FlowyError> {
  let request = data.into_inner();
  let documents: Vec<insights::DocumentInsight> =
    request.documents.into_iter().map(Into::into).collect();

  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|duration| duration.as_millis() as i64)
    .unwrap_or(0);

  let stats = insights::compute_writing_stats(&documents, now);
  data_result_ok(WritingStatsPB {
    total_documents: stats.total_documents as i64,
    documents_last_7_days: stats.documents_last_7_days as i64,
    streak_days: stats.streak_days as i64,
    words_written: stats.words_written as i64,
    top_tags: stats.top_tags.into_iter().map(Into::into).collect(),
    top_topics: stats.top_topics.into_iter().map(Into::into).collect(),
    summary_line: stats.summary_line,
  })
}

#[tracing::instrument(level = "debug", skip_all, err)]
pub(crate) async fn smart_search_handler(
  data: AFPluginData<SmartSearchRequestPB>,
) -> DataResult<RepeatedSearchHitPB, FlowyError> {
  let request = data.into_inner();
  let limit = clamp_limit(request.limit, DEFAULT_LIMIT, 50);
  let documents: Vec<search::SearchDocument> =
    request.documents.into_iter().map(Into::into).collect();

  let items = search::smart_search(&request.query, &documents, limit)
    .into_iter()
    .map(search_hit_to_pb)
    .collect();

  data_result_ok(RepeatedSearchHitPB { items })
}

#[tracing::instrument(level = "debug", skip_all, err)]
pub(crate) async fn related_documents_handler(
  data: AFPluginData<RelatedDocumentsRequestPB>,
) -> DataResult<RepeatedSearchHitPB, FlowyError> {
  let request = data.into_inner();
  let limit = clamp_limit(request.limit, 5, 20);
  let documents: Vec<search::SearchDocument> =
    request.documents.into_iter().map(Into::into).collect();
  let document: search::SearchDocument = request.document.into();

  let items = search::related_documents(&document, &documents, limit)
    .into_iter()
    .map(search_hit_to_pb)
    .collect();

  data_result_ok(RepeatedSearchHitPB { items })
}

fn search_hit_to_pb(hit: search::SearchHit) -> SearchHitPB {
  SearchHitPB {
    id: hit.id,
    title: hit.title,
    snippet: hit.snippet,
    score_milli: (hit.score * 1000.0).round() as i64,
    reason: hit.reason,
    matched_terms: hit.matched_terms,
  }
}
