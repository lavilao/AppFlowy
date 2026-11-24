//! Extractive summarization and reading time.
//!
//! The summary is produced locally by scoring sentences with term frequency,
//! title overlap, lead position, concrete figures and a redundancy filter.
//! It is intentionally extractive: the output is always the user's own words,
//! which is what makes it safe to run without a model.

use crate::text::{
  jaccard, reading_minutes, sentences, tokenize, tokenize_with_min_len, truncate_chars,
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
  pub bullets: Vec<String>,
  pub reading_minutes: u32,
}

const MAX_BULLET_CHARS: usize = 210;

/// Summarizes `body` into at most `max_bullets` non-redundant key points.
pub fn summarize(title: &str, body: &str, max_bullets: usize) -> Summary {
  let reading = reading_minutes(body);
  if max_bullets == 0 {
    return Summary {
      bullets: Vec::new(),
      reading_minutes: reading,
    };
  }

  let cleaned = collapse_whitespace(body);
  if cleaned.is_empty() {
    return Summary {
      bullets: Vec::new(),
      reading_minutes: reading,
    };
  }

  let candidates: Vec<String> = sentences(&cleaned)
    .into_iter()
    .filter(|sentence| sentence.chars().count() >= 28)
    .collect();

  if candidates.is_empty() {
    return Summary {
      bullets: vec![truncate_chars(&cleaned, 200)],
      reading_minutes: reading,
    };
  }

  if candidates.len() <= max_bullets {
    return Summary {
      bullets: candidates
        .iter()
        .map(|sentence| truncate_chars(sentence, MAX_BULLET_CHARS))
        .collect(),
      reading_minutes: reading,
    };
  }

  let mut frequencies: HashMap<String, usize> = HashMap::new();
  let tokenized: Vec<HashSet<String>> = candidates
    .iter()
    .map(|sentence| {
      let tokens = tokenize_with_min_len(sentence, 3);
      for token in &tokens {
        *frequencies.entry(token.clone()).or_insert(0) += 1;
      }
      tokens.into_iter().collect()
    })
    .collect();

  let max_frequency = frequencies.values().copied().max().unwrap_or(1).max(1) as f32;
  let title_tokens: HashSet<String> = tokenize(title).into_iter().collect();

  let mut scored: Vec<(usize, f32)> = candidates
    .iter()
    .enumerate()
    .map(|(index, sentence)| {
      let tokens = &tokenized[index];
      let mut score: f32 = tokens
        .iter()
        .map(|token| frequencies.get(token).copied().unwrap_or(0) as f32 / max_frequency)
        .sum();

      if !tokens.is_empty() {
        score /= (tokens.len() as f32).sqrt();
      }

      if !title_tokens.is_empty() {
        score += tokens.intersection(&title_tokens).count() as f32 * 0.65;
      }

      if index == 0 {
        score += 0.45;
      }
      if index == candidates.len() - 1 {
        score += 0.15;
      }
      if has_concrete_figure(sentence) {
        score += 0.25;
      }
      if sentence.chars().count() > 240 {
        score *= 0.85;
      }

      (index, score)
    })
    .collect();

  scored.sort_by(|a, b| {
    b.1
      .partial_cmp(&a.1)
      .unwrap_or(std::cmp::Ordering::Equal)
      .then_with(|| a.0.cmp(&b.0))
  });

  let mut picked: Vec<(usize, &HashSet<String>)> = Vec::new();
  for (index, _) in &scored {
    if picked.len() >= max_bullets {
      break;
    }
    let too_similar = picked
      .iter()
      .any(|(existing, tokens)| jaccard(&tokenized[*existing], tokens) >= 0.55);
    if !too_similar {
      picked.push((*index, &tokenized[*index]));
    }
  }

  picked.sort_by_key(|(index, _)| *index);

  Summary {
    bullets: picked
      .into_iter()
      .map(|(index, _)| truncate_chars(&candidates[index], MAX_BULLET_CHARS))
      .collect(),
    reading_minutes: reading,
  }
}

fn has_concrete_figure(sentence: &str) -> bool {
  sentence
    .split(|c: char| c.is_whitespace())
    .any(|word| word.chars().any(|c| c.is_ascii_digit()))
}

fn collapse_whitespace(input: &str) -> String {
  input.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn summarizes_into_requested_number_of_bullets() {
    let body = "The launch is planned for March. \
      We shipped the editor improvements last week, which reduced load time by 40 percent. \
      The team agreed to prioritize offline sync because users keep asking for it. \
      Marketing will prepare the announcement while engineering finishes the migration. \
      Budget for the quarter was approved on 2026-02-11.";
    let summary = summarize("Launch plan", body, 3);
    assert!(summary.bullets.len() <= 3);
    assert!(!summary.bullets.is_empty());
    assert!(summary.reading_minutes >= 1);
  }

  #[test]
  fn prefers_the_most_informative_sentence() {
    let body = "Nothing much happened in the garden. \
      Revenue grew by 23 percent after the pricing change. \
      We also bought some bread.";
    let summary = summarize("Business update", body, 1);
    assert_eq!(summary.bullets.len(), 1);
    assert!(summary.bullets[0].contains("Revenue"));
  }

  #[test]
  fn handles_empty_and_short_bodies() {
    assert!(summarize("Title", "", 3).bullets.is_empty());
    let short = summarize("Title", "A single short note.", 3);
    assert_eq!(short.bullets.len(), 1);
  }

  #[test]
  fn removes_near_duplicate_bullets() {
    let body = "We must ship offline mode because users asked for offline mode repeatedly. \
      Users asked for offline mode repeatedly because they work offline. \
      The team agreed that offline mode matters for commuters. \
      The design team finished the new settings screen.";
    let summary = summarize("Offline", body, 3);
    assert!(summary.bullets.len() <= 2);
  }
}
