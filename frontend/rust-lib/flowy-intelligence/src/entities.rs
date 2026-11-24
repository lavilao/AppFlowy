use flowy_derive::{ProtoBuf, ProtoBuf_Enum};
use flowy_intelligence_core::entities::EntityKind;
use flowy_intelligence_core::insights::CountedLabel;
use flowy_intelligence_core::search::SearchDocument;

#[derive(Debug, Default, Clone, ProtoBuf_Enum, PartialEq, Eq, Copy)]
pub enum EntityKindPB {
  #[default]
  Person = 0,
  Place = 1,
  Organization = 2,
  Project = 3,
  Topic = 4,
  Url = 5,
  Email = 6,
  DateTime = 7,
}

impl From<EntityKind> for EntityKindPB {
  fn from(kind: EntityKind) -> Self {
    match kind {
      EntityKind::Person => EntityKindPB::Person,
      EntityKind::Place => EntityKindPB::Place,
      EntityKind::Organization => EntityKindPB::Organization,
      EntityKind::Project => EntityKindPB::Project,
      EntityKind::Topic => EntityKindPB::Topic,
      EntityKind::Url => EntityKindPB::Url,
      EntityKind::Email => EntityKindPB::Email,
      EntityKind::DateTime => EntityKindPB::DateTime,
    }
  }
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct SummarizeRequestPB {
  #[pb(index = 1)]
  pub title: String,

  #[pb(index = 2)]
  pub text: String,

  #[pb(index = 3, one_of)]
  pub max_bullets: Option<i64>,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct SummaryPB {
  #[pb(index = 1)]
  pub bullets: Vec<String>,

  #[pb(index = 2)]
  pub reading_minutes: i64,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct ActionItemsRequestPB {
  #[pb(index = 1)]
  pub title: String,

  #[pb(index = 2)]
  pub body: String,

  /// Optional voice memo transcripts.
  #[pb(index = 3)]
  pub transcripts: Vec<String>,

  #[pb(index = 4, one_of)]
  pub max_items: Option<i64>,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct ActionItemPB {
  #[pb(index = 1)]
  pub title: String,

  #[pb(index = 2, one_of)]
  pub due_in_minutes: Option<i64>,

  /// `note` or `voice`.
  #[pb(index = 3)]
  pub source: String,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct RepeatedActionItemPB {
  #[pb(index = 1)]
  pub items: Vec<ActionItemPB>,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct TitleRequestPB {
  #[pb(index = 1)]
  pub text: String,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct TitleSuggestionPB {
  /// Empty when no suggestion could be made.
  #[pb(index = 1)]
  pub title: String,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct ExtractEntitiesRequestPB {
  #[pb(index = 1)]
  pub title: String,

  #[pb(index = 2)]
  pub text: String,

  #[pb(index = 3, one_of)]
  pub limit: Option<i64>,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct EntityPB {
  #[pb(index = 1)]
  pub display_name: String,

  #[pb(index = 2)]
  pub kind: EntityKindPB,

  #[pb(index = 3)]
  pub mentions: i64,

  /// Confidence in percent (0-100); the protobuf layer has no float type.
  #[pb(index = 4)]
  pub confidence_percent: i64,

  #[pb(index = 5)]
  pub offset_start: i64,

  #[pb(index = 6)]
  pub offset_end: i64,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct RepeatedEntityPB {
  #[pb(index = 1)]
  pub items: Vec<EntityPB>,
}

/// A document as the engine sees it. `body` should be plain text.
#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct IntelligenceDocumentPB {
  #[pb(index = 1)]
  pub id: String,

  #[pb(index = 2)]
  pub title: String,

  #[pb(index = 3)]
  pub body: String,

  #[pb(index = 4)]
  pub tags: Vec<String>,

  /// Last edit time in milliseconds since the Unix epoch.
  #[pb(index = 5)]
  pub updated_at: i64,
}

impl From<IntelligenceDocumentPB> for SearchDocument {
  fn from(value: IntelligenceDocumentPB) -> Self {
    SearchDocument {
      id: value.id,
      title: value.title,
      body: value.body,
      tags: value.tags,
      updated_at: value.updated_at,
    }
  }
}

impl From<IntelligenceDocumentPB> for flowy_intelligence_core::insights::DocumentInsight {
  fn from(value: IntelligenceDocumentPB) -> Self {
    flowy_intelligence_core::insights::DocumentInsight {
      id: value.id,
      title: value.title,
      body: value.body,
      tags: value.tags,
      updated_at: value.updated_at,
    }
  }
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct GraphRequestPB {
  #[pb(index = 1)]
  pub documents: Vec<IntelligenceDocumentPB>,

  #[pb(index = 2, one_of)]
  pub limit: Option<i64>,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct EntityHubPB {
  #[pb(index = 1)]
  pub id: String,

  #[pb(index = 2)]
  pub label: String,

  #[pb(index = 3)]
  pub kind: EntityKindPB,

  #[pb(index = 4)]
  pub mentions: i64,

  #[pb(index = 5)]
  pub connections: i64,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct RepeatedEntityHubPB {
  #[pb(index = 1)]
  pub items: Vec<EntityHubPB>,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct InsightsRequestPB {
  #[pb(index = 1)]
  pub documents: Vec<IntelligenceDocumentPB>,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct CountedLabelPB {
  #[pb(index = 1)]
  pub label: String,

  #[pb(index = 2)]
  pub count: i64,
}

impl From<CountedLabel> for CountedLabelPB {
  fn from(value: CountedLabel) -> Self {
    CountedLabelPB {
      label: value.label,
      count: value.count as i64,
    }
  }
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct WritingStatsPB {
  #[pb(index = 1)]
  pub total_documents: i64,

  #[pb(index = 2)]
  pub documents_last_7_days: i64,

  #[pb(index = 3)]
  pub streak_days: i64,

  #[pb(index = 4)]
  pub words_written: i64,

  #[pb(index = 5)]
  pub top_tags: Vec<CountedLabelPB>,

  #[pb(index = 6)]
  pub top_topics: Vec<CountedLabelPB>,

  /// Short, already formatted summary such as `3-day streak · 5 notes this week`.
  #[pb(index = 7)]
  pub summary_line: String,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct SmartSearchRequestPB {
  #[pb(index = 1)]
  pub query: String,

  #[pb(index = 2)]
  pub documents: Vec<IntelligenceDocumentPB>,

  #[pb(index = 3, one_of)]
  pub limit: Option<i64>,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct SearchHitPB {
  #[pb(index = 1)]
  pub id: String,

  #[pb(index = 2)]
  pub title: String,

  #[pb(index = 3)]
  pub snippet: String,

  /// Relevance score multiplied by 1000; the protobuf layer has no float type.
  #[pb(index = 4)]
  pub score_milli: i64,

  /// `title`, `tag`, `phrase`, `content` or `related`.
  #[pb(index = 5)]
  pub reason: String,

  #[pb(index = 6)]
  pub matched_terms: Vec<String>,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct RepeatedSearchHitPB {
  #[pb(index = 1)]
  pub items: Vec<SearchHitPB>,
}

#[derive(Debug, Default, Clone, ProtoBuf)]
pub struct RelatedDocumentsRequestPB {
  #[pb(index = 1)]
  pub document: IntelligenceDocumentPB,

  #[pb(index = 2)]
  pub documents: Vec<IntelligenceDocumentPB>,

  #[pb(index = 3, one_of)]
  pub limit: Option<i64>,
}
