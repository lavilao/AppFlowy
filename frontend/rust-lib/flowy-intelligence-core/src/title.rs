//! Title suggestions for untitled or vaguely titled documents.

use crate::text::truncate_chars;

const MAX_TITLE_CHARS: usize = 44;

/// Suggests a concise, natural title using the first meaningful line of the
/// body. Returns `None` when there is nothing useful to work with.
pub fn suggest_title(body: &str) -> Option<String> {
  let first_line = body
    .lines()
    .map(strip_markdown_prefix)
    .find(|line| line.chars().count() >= 12)?;

  let first_clause = first_line
    .split(['.', '!', '?', '\n'])
    .map(str::trim)
    .find(|clause| clause.chars().count() >= 8)
    .unwrap_or(first_line);

  let cleaned = first_clause
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ");
  if cleaned.is_empty() {
    return None;
  }

  if cleaned.chars().count() <= MAX_TITLE_CHARS {
    return Some(capitalize_first(&cleaned));
  }

  let truncated = truncate_chars(&cleaned, MAX_TITLE_CHARS + 1);
  let without_ellipsis = truncated.trim_end_matches('…').trim();
  let cut = without_ellipsis
    .rsplit_once(' ')
    .map(|(head, _)| head)
    .unwrap_or(without_ellipsis);
  Some(format!("{}…", cut.trim_end_matches([',', ';', ':']).trim()))
}

fn strip_markdown_prefix(line: &str) -> &str {
  line
    .trim_start_matches(['#', '*', '>', '`', '-', ' ', '\t'])
    .trim()
}

fn capitalize_first(input: &str) -> String {
  let mut chars = input.chars();
  match chars.next() {
    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    None => String::new(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn suggests_short_title_from_first_sentence() {
    let title = suggest_title("## The migration plan\nWe will move the database next week.");
    assert_eq!(title.as_deref(), Some("The migration plan"));
  }

  #[test]
  fn truncates_long_titles_at_a_word_boundary() {
    let body = "This is a very long opening sentence that keeps going and going without any early punctuation";
    let title = suggest_title(body).unwrap();
    assert!(title.chars().count() <= MAX_TITLE_CHARS + 1);
    assert!(title.ends_with('…'));
  }

  #[test]
  fn ignores_short_or_empty_bodies() {
    assert_eq!(suggest_title(""), None);
    assert_eq!(suggest_title("short"), None);
  }

  #[test]
  fn skips_markdown_noise_lines() {
    let title = suggest_title("# \n---\nActual content worth titling").unwrap();
    assert_eq!(title, "Actual content worth titling");
  }
}
