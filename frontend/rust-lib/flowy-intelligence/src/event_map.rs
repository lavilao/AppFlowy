use strum_macros::Display;

use flowy_derive::{Flowy_Event, ProtoBuf_Enum};
use lib_dispatch::prelude::AFPlugin;

use crate::event_handler::*;

pub fn init() -> AFPlugin {
  AFPlugin::new()
    .name(env!("CARGO_PKG_NAME"))
    .event(IntelligenceEvent::Summarize, summarize_handler)
    .event(
      IntelligenceEvent::ExtractActionItems,
      extract_action_items_handler,
    )
    .event(IntelligenceEvent::SuggestTitle, suggest_title_handler)
    .event(IntelligenceEvent::ExtractEntities, extract_entities_handler)
    .event(IntelligenceEvent::EntityHubs, entity_hubs_handler)
    .event(IntelligenceEvent::WritingStats, writing_stats_handler)
    .event(IntelligenceEvent::SmartSearch, smart_search_handler)
    .event(
      IntelligenceEvent::RelatedDocuments,
      related_documents_handler,
    )
}

/// Local, deterministic intelligence. Every event in this plugin runs offline:
/// no account, no API key and no model download is required.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Display, Hash, ProtoBuf_Enum, Flowy_Event)]
#[event_err = "FlowyError"]
pub enum IntelligenceEvent {
  #[event(input = "SummarizeRequestPB", output = "SummaryPB")]
  Summarize = 0,

  #[event(input = "ActionItemsRequestPB", output = "RepeatedActionItemPB")]
  ExtractActionItems = 1,

  #[event(input = "TitleRequestPB", output = "TitleSuggestionPB")]
  SuggestTitle = 2,

  #[event(input = "ExtractEntitiesRequestPB", output = "RepeatedEntityPB")]
  ExtractEntities = 3,

  #[event(input = "GraphRequestPB", output = "RepeatedEntityHubPB")]
  EntityHubs = 4,

  #[event(input = "InsightsRequestPB", output = "WritingStatsPB")]
  WritingStats = 5,

  #[event(input = "SmartSearchRequestPB", output = "RepeatedSearchHitPB")]
  SmartSearch = 6,

  #[event(input = "RelatedDocumentsRequestPB", output = "RepeatedSearchHitPB")]
  RelatedDocuments = 7,
}
