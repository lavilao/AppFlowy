//! Explainable smart search and related-document discovery.
//!
//! Both use BM25 over the user's documents plus explicit signals (title, tags,
//! phrase and recency). Every hit reports *why* it matched, so the UI can show
//! a short, honest reason instead of a black-box relevance number.

use crate::keywords::Bm25Index;
use crate::text::{normalize, search_tokens, sentences, truncate_chars};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
pub struct SearchDocument {
  pub id: String,
  pub title: String,
  pub body: String,
  pub tags: Vec<String>,
  /// Last edit time in milliseconds since the Unix epoch; breaks ties.
  pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
  pub id: String,
  pub title: String,
  pub snippet: String,
  pub score: f32,
  /// Why the document surfaced: `title`, `tag`, `phrase` or `content`.
  pub reason: String,
  /// The query terms that actually matched, for highlighting.
  pub matched_terms: Vec<String>,
}

const SNIPPET_CHARS: usize = 160;

/// Ranks documents for `query`.
pub fn smart_search(query: &str, documents: &[SearchDocument], limit: usize) -> Vec<SearchHit> {
  let mut query_tokens = search_tokens(query);
  query_tokens.dedup();
  let phrase = normalize(query)
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ");

  if limit == 0 || documents.is_empty() {
    return Vec::new();
  }
  if query_tokens.is_empty() && phrase.is_empty() {
    return Vec::new();
  }

  rank(documents, &query_tokens, &phrase, None, limit, "content")
}

/// Finds documents that are about the same things as `document`, using its
/// most distinctive terms as the query.
pub fn related_documents(
  document: &SearchDocument,
  documents: &[SearchDocument],
  limit: usize,
) -> Vec<SearchHit> {
  if limit == 0 || documents.len() <= 1 {
    return Vec::new();
  }

  let corpus: Vec<Vec<String>> = documents
    .iter()
    .map(|candidate| search_tokens(&candidate_text(candidate)))
    .collect();
  let own_index = documents
    .iter()
    .position(|candidate| candidate.id == document.id);
  let own_tokens = match own_index {
    Some(index) => corpus[index].clone(),
    None => search_tokens(&candidate_text(document)),
  };

  let distinctive = crate::keywords::tfidf(&own_tokens, &corpus, 8);
  let mut query_tokens: Vec<String> = distinctive.into_iter().map(|scored| scored.term).collect();
  query_tokens.extend(search_tokens(&document.title));
  query_tokens.dedup();

  let phrase = String::new();
  rank(
    documents,
    &query_tokens,
    &phrase,
    Some(document.id.as_str()),
    limit,
    "related",
  )
}

fn rank(
  documents: &[SearchDocument],
  query_tokens: &[String],
  phrase: &str,
  exclude_id: Option<&str>,
  limit: usize,
  default_reason: &str,
) -> Vec<SearchHit> {
  let corpus: Vec<Vec<String>> = documents
    .iter()
    .map(|document| search_tokens(&candidate_text(document)))
    .collect();
  let index = Bm25Index::new(&corpus);

  let newest = documents
    .iter()
    .map(|document| document.updated_at)
    .max()
    .unwrap_or(0);
  let oldest = documents
    .iter()
    .map(|document| document.updated_at)
    .min()
    .unwrap_or(0);
  let span = (newest - oldest).max(1) as f32;

  let mut hits: Vec<SearchHit> = Vec::new();

  for (position, document) in documents.iter().enumerate() {
    if exclude_id.is_some_and(|id| id == document.id) {
      continue;
    }

    let tokens = &corpus[position];
    let token_set: HashSet<&str> = tokens.iter().map(String::as_str).collect();

    let matched_terms: Vec<String> = query_tokens
      .iter()
      .filter(|query| token_set.iter().any(|token| term_matches(query, token)))
      .cloned()
      .collect();

    let phrase_hit = !phrase.is_empty()
      && phrase.contains(' ')
      && normalize(&format!("{} {}", document.title, document.body)).contains(phrase);

    if matched_terms.is_empty() && !phrase_hit {
      continue;
    }

    let title_tokens: HashSet<String> = search_tokens(&document.title).into_iter().collect();
    let title_hits = matched_terms
      .iter()
      .filter(|term| title_tokens.contains(*term))
      .count() as f32;

    let tag_tokens: HashSet<String> = document
      .tags
      .iter()
      .flat_map(|tag| search_tokens(tag))
      .collect();
    let tag_hits = matched_terms
      .iter()
      .filter(|term| tag_tokens.contains(*term))
      .count() as f32;

    let mut score = index.score(query_tokens, tokens) as f32;
    score += title_hits * 2.0;
    score += tag_hits * 1.5;
    if phrase_hit {
      score += 2.5;
    }
    if document.updated_at > 0 {
      score += ((document.updated_at - oldest) as f32 / span) * 0.15;
    }
    if score <= 0.0 {
      continue;
    }

    let reason = if title_hits > 0.0 {
      "title".to_string()
    } else if tag_hits > 0.0 {
      "tag".to_string()
    } else if phrase_hit {
      "phrase".to_string()
    } else {
      default_reason.to_string()
    };

    hits.push(SearchHit {
      id: document.id.clone(),
      title: document.title.clone(),
      snippet: best_snippet(document, &matched_terms),
      score,
      reason,
      matched_terms,
    });
  }

  hits.sort_by(|a, b| {
    b.score
      .partial_cmp(&a.score)
      .unwrap_or(std::cmp::Ordering::Equal)
      .then_with(|| a.id.cmp(&b.id))
  });
  hits.truncate(limit);
  hits
}

fn candidate_text(document: &SearchDocument) -> String {
  format!(
    "{} {} {}",
    document.title,
    document.body,
    document.tags.join(" ")
  )
}

fn term_matches(query: &str, document_term: &str) -> bool {
  if query == document_term {
    return true;
  }
  let query_len = query.chars().count();
  let term_len = document_term.chars().count();
  query_len >= 4
    && term_len >= 4
    && (document_term.starts_with(query) || query.starts_with(document_term))
}

fn best_snippet(document: &SearchDocument, matched_terms: &[String]) -> String {
  if matched_terms.is_empty() {
    return truncate_chars(&document.body, SNIPPET_CHARS);
  }

  let mut best: Option<(usize, String)> = None;
  for sentence in sentences(&document.body) {
    let lowered = normalize(&sentence);
    let score = matched_terms
      .iter()
      .filter(|term| lowered.contains(term.as_str()))
      .count();
    if score > 0 && best.as_ref().is_none_or(|(current, _)| score > *current) {
      best = Some((score, sentence));
    }
  }

  match best {
    Some((_, sentence)) => truncate_chars(&sentence, SNIPPET_CHARS),
    None => truncate_chars(&document.body, SNIPPET_CHARS),
  }
}

/// Frequency map used by the UI to de-emphasise repeated query terms.
pub fn term_hits(hits: &[SearchHit]) -> HashMap<String, u32> {
  let mut counts = HashMap::new();
  for hit in hits {
    for term in &hit.matched_terms {
      *counts.entry(term.clone()).or_insert(0) += 1;
    }
  }
  counts
}

#[cfg(test)]
mod tests {
  use super::*;

  fn document(id: &str, title: &str, body: &str, tags: &[&str], updated_at: i64) -> SearchDocument {
    SearchDocument {
      id: id.to_string(),
      title: title.to_string(),
      body: body.to_string(),
      tags: tags.iter().map(|tag| tag.to_string()).collect(),
      updated_at,
    }
  }

  fn workspace() -> Vec<SearchDocument> {
    vec![
      document(
        "1",
        "Quarterly plan",
        "Revenue grew by 23 percent after the pricing change.",
        &["work", "finance"],
        1_000,
      ),
      document(
        "2",
        "Trip notes",
        "We hiked near Kyoto and visited an old temple.",
        &["travel"],
        2_000,
      ),
      document(
        "3",
        "Pricing draft",
        "Draft pricing for the new tier, pending review.",
        &["work"],
        3_000,
      ),
    ]
  }

  #[test]
  fn finds_documents_by_topic() {
    let hits = smart_search("pricing", &workspace(), 5);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].id, "3");
    assert_eq!(hits[0].reason, "title");
  }

  #[test]
  fn ignores_accents_and_case() {
    let documents = vec![document("1", "Día de campo", "Fuimos al campo", &[], 0)];
    let hits = smart_search("DIA", &documents, 5);
    assert_eq!(hits.len(), 1);
  }

  #[test]
  fn handles_short_queries() {
    let documents = vec![document("1", "AI notes", "Notes about AI", &[], 0)];
    assert_eq!(smart_search("ai", &documents, 5).len(), 1);
  }

  #[test]
  fn returns_nothing_for_unmatched_queries() {
    assert!(smart_search("astrophysics", &workspace(), 5).is_empty());
    assert!(smart_search("", &workspace(), 5).is_empty());
    assert!(smart_search("pricing", &workspace(), 0).is_empty());
  }

  #[test]
  fn explains_why_a_hit_matched() {
    let hits = smart_search("travel", &workspace(), 5);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].reason, "tag");
  }

  #[test]
  fn related_documents_exclude_self_and_share_topics() {
    let documents = workspace();
    let related = related_documents(&documents[2], &documents, 5);
    assert!(related.iter().all(|hit| hit.id != "3"));
    assert!(related.iter().any(|hit| hit.id == "1"));
  }

  #[test]
  fn snippet_comes_from_a_matching_sentence() {
    let hits = smart_search("pricing", &workspace(), 5);
    let hit = hits.iter().find(|hit| hit.id == "1").unwrap();
    assert!(hit.snippet.contains("pricing"));
  }
}
