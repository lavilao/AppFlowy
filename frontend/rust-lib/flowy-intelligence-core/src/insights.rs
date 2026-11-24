//! Workspace insights: writing activity, streaks, tags and recurring topics.
//!
//! AppFlowy has no separate "journal" screen, so insights are computed from the
//! documents the user already has. They stay a thin, optional summary rather
//! than a new dashboard.

use crate::keywords::{term_frequencies, tfidf};
use crate::text::tokenize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
pub struct DocumentInsight {
  pub id: String,
  pub title: String,
  pub body: String,
  pub tags: Vec<String>,
  /// Last edit time in milliseconds since the Unix epoch.
  pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountedLabel {
  pub label: String,
  pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritingStats {
  pub total_documents: u32,
  pub documents_last_7_days: u32,
  pub streak_days: u32,
  pub words_written: u32,
  pub top_tags: Vec<CountedLabel>,
  pub top_topics: Vec<CountedLabel>,
  pub summary_line: String,
}

const MILLIS_PER_DAY: i64 = 24 * 60 * 60 * 1000;

/// Computes activity statistics. `now_millis` is injected so the result is
/// deterministic in tests.
pub fn compute_writing_stats(documents: &[DocumentInsight], now_millis: i64) -> WritingStats {
  if documents.is_empty() {
    return WritingStats {
      total_documents: 0,
      documents_last_7_days: 0,
      streak_days: 0,
      words_written: 0,
      top_tags: Vec::new(),
      top_topics: Vec::new(),
      summary_line: "No notes yet".to_string(),
    };
  }

  let today = day_index(now_millis);
  let week_ago = today - 6;

  let days: HashSet<i64> = documents
    .iter()
    .filter(|document| document.updated_at > 0)
    .map(|document| day_index(document.updated_at))
    .collect();

  let documents_last_7_days = documents
    .iter()
    .filter(|document| day_index(document.updated_at) >= week_ago)
    .count() as u32;

  let streak_days = compute_streak(&days, today);
  let words_written = documents
    .iter()
    .map(|document| document.body.split_whitespace().count() as u32)
    .sum();

  let mut tag_counts: HashMap<String, u32> = HashMap::new();
  for tag in documents.iter().flat_map(|document| document.tags.iter()) {
    let label = tag.trim();
    if !label.is_empty() {
      *tag_counts.entry(label.to_string()).or_insert(0) += 1;
    }
  }
  let mut top_tags: Vec<CountedLabel> = tag_counts
    .into_iter()
    .map(|(label, count)| CountedLabel { label, count })
    .collect();
  top_tags.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));
  top_tags.truncate(5);

  let top_topics = recurring_topics(documents, 5);

  let mut parts = Vec::new();
  if streak_days >= 2 {
    parts.push(format!("{streak_days}-day streak"));
  }
  if documents_last_7_days > 0 {
    parts.push(format!(
      "{documents_last_7_days} {} this week",
      if documents_last_7_days == 1 {
        "note"
      } else {
        "notes"
      }
    ));
  } else {
    parts.push(format!("{} notes", documents.len()));
  }

  WritingStats {
    total_documents: documents.len() as u32,
    documents_last_7_days,
    streak_days,
    words_written,
    top_tags,
    top_topics,
    summary_line: parts.join(" · "),
  }
}

fn day_index(millis: i64) -> i64 {
  millis.div_euclid(MILLIS_PER_DAY)
}

fn compute_streak(days: &HashSet<i64>, today: i64) -> u32 {
  if days.is_empty() {
    return 0;
  }
  // Writing later today is optional: anchor the streak on yesterday when the
  // user has not written yet.
  let mut cursor = if days.contains(&today) {
    today
  } else {
    today - 1
  };
  let mut streak = 0u32;
  while streak < 365 && days.contains(&cursor) {
    streak += 1;
    cursor -= 1;
  }
  streak
}

/// Topics are corpus-wide TF-IDF terms, so generic words shared by every
/// document do not dominate the list.
fn recurring_topics(documents: &[DocumentInsight], limit: usize) -> Vec<CountedLabel> {
  let corpus: Vec<Vec<String>> = documents
    .iter()
    .map(|document| tokenize(&format!("{} {}", document.title, document.body)))
    .collect();

  let mut scores: HashMap<String, f64> = HashMap::new();
  for (index, document) in documents.iter().enumerate() {
    let tokens = corpus.get(index).cloned().unwrap_or_default();
    let ranked = tfidf(&tokens, &corpus, 10);

    let mut seen_in_document = HashSet::new();
    for scored in ranked {
      if seen_in_document.insert(scored.term.clone()) {
        *scores.entry(scored.term).or_insert(0.0) += scored.score;
      }
    }
    let _ = document;
  }

  let mut ranked: Vec<CountedLabel> = scores
    .into_iter()
    .map(|(label, score)| CountedLabel {
      label,
      count: score.round().max(1.0) as u32,
    })
    .collect();
  ranked.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));
  ranked.truncate(limit);
  ranked
}

/// Frequency of every term across the workspace, useful for autocomplete and
/// for the search "suggestions" without a network round trip.
pub fn term_cloud(documents: &[DocumentInsight], limit: usize) -> Vec<CountedLabel> {
  let tokens: Vec<String> = documents
    .iter()
    .flat_map(|document| tokenize(&format!("{} {}", document.title, document.body)))
    .collect();
  let mut cloud: Vec<CountedLabel> = term_frequencies(&tokens)
    .into_iter()
    .map(|(label, count)| CountedLabel {
      label,
      count: count as u32,
    })
    .collect();
  cloud.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));
  cloud.truncate(limit);
  cloud
}

#[cfg(test)]
mod tests {
  use super::*;

  const DAY: i64 = MILLIS_PER_DAY;

  fn document(id: &str, body: &str, tags: &[&str], day: i64) -> DocumentInsight {
    DocumentInsight {
      id: id.to_string(),
      title: format!("Note {id}"),
      body: body.to_string(),
      tags: tags.iter().map(|tag| tag.to_string()).collect(),
      updated_at: day * DAY,
    }
  }

  #[test]
  fn empty_workspace_has_no_stats() {
    let stats = compute_writing_stats(&[], 0);
    assert_eq!(stats.total_documents, 0);
    assert_eq!(stats.streak_days, 0);
  }

  #[test]
  fn counts_recent_notes_and_streaks() {
    let today = 20_000;
    let documents = vec![
      document("1", "alpha", &["work"], today),
      document("2", "beta", &["work", "ideas"], today - 1),
      document("3", "gamma", &["ideas"], today - 2),
      document("4", "delta", &[], today - 30),
    ];
    let stats = compute_writing_stats(&documents, today * DAY + 1000);
    assert_eq!(stats.total_documents, 4);
    assert_eq!(stats.documents_last_7_days, 3);
    assert_eq!(stats.streak_days, 3);
    assert_eq!(
      stats.top_tags[0],
      CountedLabel {
        label: "ideas".into(),
        count: 2
      }
    );
    assert!(stats.summary_line.contains("3-day streak"));
  }

  #[test]
  fn streak_survives_a_day_without_writing_yet() {
    let today = 5_000;
    let documents = vec![
      document("1", "alpha", &[], today - 1),
      document("2", "beta", &[], today - 2),
    ];
    let stats = compute_writing_stats(&documents, today * DAY);
    assert_eq!(stats.streak_days, 2);
  }

  #[test]
  fn topics_skip_workspace_wide_words() {
    let documents = vec![
      document("1", "project alpha alpha", &[], 1),
      document("2", "project beta beta", &[], 1),
    ];
    let topics = recurring_topics(&documents, 2);
    assert!(topics.iter().any(|topic| topic.label == "alpha"));
  }
}
