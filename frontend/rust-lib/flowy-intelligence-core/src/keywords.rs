//! Keyword extraction with TF-IDF and BM25 scoring.
//!
//! Both are deterministic, explainable and need no training data, which keeps
//! the "smart" behaviour usable offline and reproducible in tests.

use crate::text::tokenize;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub struct ScoredTerm {
  pub term: String,
  pub score: f64,
}

/// Counts how often each term appears.
pub fn term_frequencies(tokens: &[String]) -> HashMap<String, usize> {
  let mut frequencies = HashMap::new();
  for token in tokens {
    *frequencies.entry(token.clone()).or_insert(0) += 1;
  }
  frequencies
}

/// Ranks the terms of `tokens` by TF-IDF against `corpus`.
///
/// `corpus` should contain the tokenized bodies of the surrounding documents so
/// a term that appears everywhere (and therefore distinguishes nothing) is
/// penalised.
pub fn tfidf(tokens: &[String], corpus: &[Vec<String>], limit: usize) -> Vec<ScoredTerm> {
  if tokens.is_empty() || limit == 0 {
    return Vec::new();
  }

  let frequencies = term_frequencies(tokens);
  let total_documents = corpus.len().max(1) as f64;

  let mut document_frequencies: HashMap<&str, usize> = HashMap::new();
  let mut seen_in_document: HashSet<&str> = HashSet::new();
  for document in corpus {
    seen_in_document.clear();
    for token in document {
      if seen_in_document.insert(token.as_str()) {
        *document_frequencies.entry(token.as_str()).or_insert(0) += 1;
      }
    }
  }

  let mut scored: Vec<ScoredTerm> = frequencies
    .into_iter()
    .map(|(term, count)| {
      let tf = count as f64 / tokens.len() as f64;
      let df = *document_frequencies.get(term.as_str()).unwrap_or(&1) as f64;
      let idf = (total_documents / df.max(1.0)).ln() + 1.0;
      ScoredTerm {
        term,
        score: tf * idf,
      }
    })
    .collect();

  scored.sort_by(|a, b| {
    b.score
      .partial_cmp(&a.score)
      .unwrap_or(std::cmp::Ordering::Equal)
      .then_with(|| a.term.cmp(&b.term))
  });
  scored.truncate(limit);
  scored
}

/// Convenience wrapper that tokenizes `text` before ranking it.
pub fn keywords_for_text(text: &str, corpus: &[String], limit: usize) -> Vec<String> {
  let tokens = tokenize(text);
  let corpus_tokens: Vec<Vec<String>> = corpus.iter().map(|doc| tokenize(doc)).collect();
  tfidf(&tokens, &corpus_tokens, limit)
    .into_iter()
    .map(|scored| scored.term)
    .collect()
}

/// Standard BM25 relevance score of `query` against `document`.
///
/// `average_document_length` and `document_frequencies` describe the corpus the
/// document belongs to; call [`Bm25Index`] to build those once.
pub fn bm25(
  query: &[String],
  document: &[String],
  average_document_length: f64,
  document_count: usize,
  document_frequencies: &HashMap<String, usize>,
  k1: f64,
  b: f64,
) -> f64 {
  if query.is_empty() || document.is_empty() {
    return 0.0;
  }

  let mut frequencies: HashMap<&str, usize> = HashMap::new();
  for token in document {
    *frequencies.entry(token.as_str()).or_insert(0) += 1;
  }

  let mut score = 0.0;
  for term in query {
    let Some(&frequency) = frequencies.get(term.as_str()) else {
      continue;
    };
    let df = *document_frequencies.get(term).unwrap_or(&1) as f64;
    let idf = ((document_count as f64 - df + 0.5) / (df + 0.5) + 1.0).ln();
    let tf = frequency as f64;
    let denominator =
      tf + k1 * (1.0 - b + b * document.len() as f64 / average_document_length.max(1.0));
    score += idf * (tf * (k1 + 1.0)) / denominator.max(1e-9);
  }
  score
}

/// Pre-computed corpus statistics for repeated BM25 queries.
pub struct Bm25Index {
  average_document_length: f64,
  document_count: usize,
  document_frequencies: HashMap<String, usize>,
}

impl Bm25Index {
  pub fn new(corpus: &[Vec<String>]) -> Self {
    let document_count = corpus.len();
    let total_length: usize = corpus.iter().map(|document| document.len()).sum();
    let average_document_length = if document_count == 0 {
      0.0
    } else {
      total_length as f64 / document_count as f64
    };

    let mut document_frequencies: HashMap<String, usize> = HashMap::new();
    for document in corpus {
      let mut seen = HashSet::new();
      for token in document {
        if seen.insert(token.as_str()) {
          *document_frequencies.entry(token.clone()).or_insert(0) += 1;
        }
      }
    }

    Self {
      average_document_length,
      document_count,
      document_frequencies,
    }
  }

  pub fn score(&self, query: &[String], document: &[String]) -> f64 {
    bm25(
      query,
      document,
      self.average_document_length,
      self.document_count,
      &self.document_frequencies,
      1.2,
      0.75,
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn tokens(text: &str) -> Vec<String> {
    tokenize(text)
  }

  #[test]
  fn ranks_distinctive_terms_higher() {
    let corpus = vec![
      tokens("alpha alpha shared"),
      tokens("beta shared"),
      tokens("gamma shared"),
    ];
    let ranked = tfidf(&tokens("alpha shared"), &corpus, 2);
    assert_eq!(ranked[0].term, "alpha");
  }

  #[test]
  fn returns_empty_for_empty_input() {
    assert!(tfidf(&[], &[], 5).is_empty());
    assert!(tfidf(&tokens("alpha"), &[], 0).is_empty());
  }

  #[test]
  fn bm25_prefers_more_matches() {
    let corpus = vec![tokens("alpha beta"), tokens("gamma delta")];
    let index = Bm25Index::new(&corpus);
    let query = tokens("beta");
    assert!(index.score(&query, &corpus[0]) > index.score(&query, &corpus[1]));
  }

  #[test]
  fn keywords_are_deterministic() {
    let corpus = vec!["project deadline notes".to_string()];
    let first = keywords_for_text("project deadline deadline notes", &corpus, 3);
    let second = keywords_for_text("project deadline deadline notes", &corpus, 3);
    assert_eq!(first, second);
    assert_eq!(first[0], "deadline");
  }
}
