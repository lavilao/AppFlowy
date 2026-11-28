//! Locale aware text primitives shared by every engine module.
//!
//! AppFlowy users write in many languages; the tokenizer therefore folds
//! accents before tokenizing so `día`, `DIA` and `dia` are the same term, and
//! it knows both English and Spanish function words.

use std::collections::HashSet;

/// Folds common accented Latin characters to their unaccented ASCII form.
pub fn fold_accents(input: &str) -> String {
  input
    .chars()
    .map(|c| match c {
      'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
      'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' | 'Ā' | 'Ă' | 'Ą' => 'A',
      'ç' | 'ć' | 'č' => 'c',
      'Ç' | 'Ć' | 'Č' => 'C',
      'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
      'É' | 'È' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => 'E',
      'í' | 'ì' | 'î' | 'ï' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
      'Í' | 'Ì' | 'Î' | 'Ï' | 'Ī' | 'Ĭ' | 'Į' | 'İ' => 'I',
      'ñ' | 'ń' | 'ň' => 'n',
      'Ñ' | 'Ń' | 'Ň' => 'N',
      'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ō' | 'ŏ' | 'ő' | 'ø' => 'o',
      'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' | 'Ō' | 'Ŏ' | 'Ő' | 'Ø' => 'O',
      'š' | 'ś' => 's',
      'Š' | 'Ś' => 'S',
      'ú' | 'ù' | 'û' | 'ü' | 'ū' | 'ŭ' | 'ů' | 'ű' => 'u',
      'Ú' | 'Ù' | 'Û' | 'Ü' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' => 'U',
      'ý' | 'ÿ' => 'y',
      'Ý' | 'Ÿ' => 'Y',
      'ž' | 'ź' | 'ż' => 'z',
      'Ž' | 'Ź' | 'Ż' => 'Z',
      other => other,
    })
    .collect()
}

/// Lowercases and folds accents. `"Mañana"` becomes `"manana"`.
pub fn normalize(input: &str) -> String {
  fold_accents(&input.to_lowercase())
}

/// The stop word list covers the languages AppFlowy is most used in. Keeping
/// it small and honest is better than a huge list that misses inflections.
const STOPWORDS: &[&str] = &[
  // English
  "the",
  "and",
  "for",
  "are",
  "but",
  "not",
  "you",
  "your",
  "yours",
  "with",
  "have",
  "has",
  "had",
  "was",
  "were",
  "been",
  "being",
  "this",
  "that",
  "these",
  "those",
  "from",
  "into",
  "about",
  "after",
  "before",
  "again",
  "against",
  "because",
  "been",
  "being",
  "both",
  "can",
  "cannot",
  "could",
  "did",
  "does",
  "doing",
  "down",
  "during",
  "each",
  "few",
  "further",
  "her",
  "here",
  "hers",
  "herself",
  "him",
  "himself",
  "his",
  "how",
  "its",
  "itself",
  "just",
  "let",
  "more",
  "most",
  "much",
  "must",
  "nor",
  "off",
  "once",
  "only",
  "other",
  "ought",
  "our",
  "ours",
  "ourselves",
  "out",
  "over",
  "own",
  "same",
  "she",
  "should",
  "some",
  "such",
  "than",
  "their",
  "theirs",
  "them",
  "themselves",
  "then",
  "there",
  "they",
  "through",
  "too",
  "under",
  "until",
  "very",
  "what",
  "when",
  "where",
  "which",
  "while",
  "who",
  "whom",
  "why",
  "will",
  "would",
  "today",
  "yesterday",
  "tomorrow",
  "also",
  "still",
  "well",
  "back",
  "even",
  "like",
  "really",
  "think",
  "thought",
  "going",
  "made",
  "make",
  "take",
  "took",
  "see",
  "saw",
  "got",
  "get",
  "day",
  // Spanish
  "del",
  "las",
  "los",
  "una",
  "uno",
  "unas",
  "unos",
  "por",
  "para",
  "con",
  "sin",
  "sobre",
  "entre",
  "desde",
  "hasta",
  "cuando",
  "donde",
  "como",
  "pero",
  "porque",
  "este",
  "esta",
  "esto",
  "estos",
  "estas",
  "esos",
  "esas",
  "aquel",
  "aquella",
  "hay",
  "fue",
  "era",
  "son",
  "ser",
  "esta",
  "estan",
  "estaba",
  "estaban",
  "muy",
  "mas",
  "menos",
  "tambien",
  "tampoco",
  "todo",
  "toda",
  "todos",
  "todas",
  "algo",
  "algun",
  "alguna",
  "algunos",
  "algunas",
  "nada",
  "ningun",
  "ninguna",
  "otro",
  "otra",
  "otros",
  "otras",
  "mismo",
  "misma",
  "solo",
  "sola",
  "ahora",
  "antes",
  "despues",
  "luego",
  "entonces",
  "aqui",
  "alli",
  "alla",
  "cada",
  "que",
  "cual",
  "quien",
  "mi",
  "mis",
  "tu",
  "tus",
  "su",
  "sus",
  "nuestro",
  "nuestra",
  "nosotros",
  "vosotros",
  "ellos",
  "ellas",
  "nos",
  "les",
  "se",
  "me",
  "te",
  "lo",
  "la",
  "el",
  "un",
  "una",
  "de",
  "en",
  "y",
  "o",
  "a",
  "al",
  "es",
  "no",
  "si",
  "ya",
  "he",
  "ha",
  "han",
  "hemos",
  "tenia",
  "tenian",
  "tiene",
  "tienen",
  "hacer",
  "hizo",
  "ser",
];

/// True when `term` (already lowercased) carries no topical meaning.
pub fn is_stopword(term: &str) -> bool {
  STOPWORDS.contains(&term)
}

/// Tokenizes into normalized, stop-word free terms of at least `min_len`
/// characters. Splitting keeps digits so dates and amounts survive.
pub fn tokenize_with_min_len(input: &str, min_len: usize) -> Vec<String> {
  normalize(input)
    .split(|c: char| !c.is_alphanumeric())
    .filter(|term| {
      let len = term.chars().count();
      len >= min_len && !is_stopword(term) && !term.chars().all(|c| c.is_numeric())
    })
    .map(|term| term.to_string())
    .collect()
}

/// Default tokenizer: terms of 3+ characters without stop words.
pub fn tokenize(input: &str) -> Vec<String> {
  tokenize_with_min_len(input, 3)
}

/// Search tokenizer: keeps 2 character terms so short queries such as `ai`
/// still match.
pub fn search_tokens(input: &str) -> Vec<String> {
  tokenize_with_min_len(input, 2)
}

/// Splits prose into sentences. A `.` only ends a sentence when it is
/// followed by whitespace or the end of the text, so `3.5` and `v1.2` survive.
pub fn sentences(text: &str) -> Vec<String> {
  let chars: Vec<char> = text.chars().collect();
  let mut out = Vec::new();
  let mut start = 0usize;
  let mut idx = 0usize;

  while idx < chars.len() {
    let current = chars[idx];
    let boundary = match current {
      '!' | '?' | '\n' => true,
      '.' => match chars.get(idx + 1) {
        None => true,
        Some(next) => next.is_whitespace() || *next == '"' || *next == '\'' || *next == '」',
      },
      _ => false,
    };

    if boundary {
      let sentence: String = chars[start..=idx].iter().collect();
      push_trimmed(&mut out, &sentence);
      start = idx + 1;
    }
    idx += 1;
  }

  if start < chars.len() {
    let sentence: String = chars[start..].iter().collect();
    push_trimmed(&mut out, &sentence);
  }

  out
}

fn push_trimmed(out: &mut Vec<String>, raw: &str) {
  let cleaned = raw
    .trim()
    .trim_start_matches(['-', '*', '•', ' ', '\t'])
    .trim();
  if !cleaned.is_empty() {
    out.push(cleaned.to_string());
  }
}

/// Number of whitespace separated words.
pub fn word_count(text: &str) -> usize {
  text.split_whitespace().count()
}

/// Estimated reading time in minutes (200 words per minute, at least one).
pub fn reading_minutes(text: &str) -> u32 {
  let words = word_count(text);
  if words == 0 {
    return 1;
  }
  ((words as f64) / 200.0).ceil().max(1.0) as u32
}

/// Truncates on a character boundary, appending an ellipsis when cut.
pub fn truncate_chars(input: &str, max_chars: usize) -> String {
  let trimmed = input.trim();
  if trimmed.chars().count() <= max_chars {
    return trimmed.to_string();
  }
  let mut out: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
  out = out.trim_end().to_string();
  out.push('…');
  out
}

/// Case insensitive lookup that returns a **character** offset, safe for
/// slicing arbitrary unicode text.
pub fn find_ci(haystack: &[char], needle: &str) -> Option<usize> {
  let needle: Vec<char> = normalize(needle).chars().collect();
  if needle.is_empty() || needle.len() > haystack.len() {
    return None;
  }
  let folded: Vec<char> = haystack
    .iter()
    .map(|c| normalize(&c.to_string()).chars().next().unwrap_or(*c))
    .collect();

  (0..=folded.len() - needle.len()).find(|&start| folded[start..start + needle.len()] == needle[..])
}

/// True when `haystack` starts with `prefix`, ignoring case and accents.
pub fn starts_with_ci(haystack: &str, prefix: &str) -> bool {
  let haystack = normalize(haystack);
  let prefix = normalize(prefix);
  haystack.starts_with(&prefix)
}

/// Jaccard similarity between two token sets, used for redundancy filtering.
pub fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f32 {
  if a.is_empty() && b.is_empty() {
    return 1.0;
  }
  let intersection = a.intersection(b).count() as f32;
  let union = a.union(b).count() as f32;
  if union == 0.0 {
    0.0
  } else {
    intersection / union
  }
}

/// A stable identifier for an entity or document node in the knowledge graph.
pub fn slug(input: &str) -> String {
  normalize(input)
    .split(|c: char| !c.is_alphanumeric())
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join("-")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn folds_accents_and_case() {
    assert_eq!(normalize("Mañana DÍA"), "manana dia");
    assert_eq!(normalize("Café au lait"), "cafe au lait");
  }

  #[test]
  fn tokenizes_without_stopwords_or_numbers() {
    let tokens = tokenize("The project with 2024 deadlines needs a plan");
    assert!(tokens.contains(&"project".to_string()));
    assert!(tokens.contains(&"deadlines".to_string()));
    assert!(!tokens.contains(&"the".to_string()));
    assert!(!tokens.contains(&"2024".to_string()));
  }

  #[test]
  fn keeps_short_terms_for_search() {
    let tokens = search_tokens("ai and ui");
    assert_eq!(tokens, vec!["ai".to_string(), "ui".to_string()]);
  }

  #[test]
  fn splits_sentences_without_breaking_decimals() {
    let items = sentences("Version 1.2 shipped. It works! Next up\nanother line");
    assert_eq!(items.len(), 4);
    assert_eq!(items[0], "Version 1.2 shipped.");
  }

  #[test]
  fn computes_reading_time() {
    assert_eq!(reading_minutes(""), 1);
    assert_eq!(reading_minutes(&"word ".repeat(401)), 3);
  }

  #[test]
  fn truncates_on_char_boundary() {
    assert_eq!(truncate_chars("hello world", 8), "hello w…");
    assert_eq!(truncate_chars("short", 20), "short");
  }

  #[test]
  fn finds_needles_ignoring_accents() {
    let haystack: Vec<char> = "Visita a la montaña rusa".chars().collect();
    assert_eq!(find_ci(&haystack, "MONTANA"), Some(12));
    assert_eq!(find_ci(&haystack, "playa"), None);
  }

  #[test]
  fn computes_jaccard() {
    let a: HashSet<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
    let b: HashSet<String> = ["b", "c"].iter().map(|s| s.to_string()).collect();
    assert!((jaccard(&a, &b) - 1.0 / 3.0).abs() < 1e-6);
  }
}
