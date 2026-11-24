//! Action item extraction from notes and voice transcripts.
//!
//! Handles explicit `- [ ]` checkboxes, `TODO:` prefixes and natural prose in
//! English and Spanish ("need to call Ana", "tengo que pagar la factura"),
//! plus a light due-date hint ("en 2 horas", "mañana").

use crate::text::{find_ci, normalize};

#[derive(Debug, Clone, PartialEq)]
pub struct ActionItem {
  pub title: String,
  /// Relative due time in minutes, when the sentence implies one.
  pub due_in_minutes: Option<i64>,
  pub source: String,
}

const EXPLICIT_PREFIXES: &[&str] = &[
  "todo",
  "to-do",
  "pendiente",
  "tarea",
  "accion",
  "acción",
  "recordatorio",
];

const PHRASES: &[&str] = &[
  "tengo que ",
  "hay que ",
  "debo ",
  "necesito ",
  "acordarme de ",
  "recuerdame ",
  "recuérdame ",
  "no olvidar ",
  "no olvidarme de ",
  "pendiente de ",
  "remember to ",
  "need to ",
  "must ",
];

const IMPERATIVES: &[&str] = &[
  "llamar",
  "comprar",
  "enviar",
  "mandar",
  "pagar",
  "revisar",
  "reservar",
  "pedir",
  "escribir",
  "terminar",
  "recoger",
  "confirmar",
  "buscar",
  "preparar",
  "agendar",
  "call",
  "buy",
  "send",
  "pay",
  "check",
  "review",
  "book",
  "email",
];

/// Extracts up to `max_items` action items. `transcripts` holds voice memo
/// transcriptions, which are scanned sentence by sentence in addition to the
/// note body.
pub fn extract_action_items(
  title: &str,
  body: &str,
  transcripts: &[String],
  max_items: usize,
) -> Vec<ActionItem> {
  if max_items == 0 {
    return Vec::new();
  }

  let mut candidates: Vec<(String, &'static str)> = body
    .lines()
    .map(|line| (line.to_string(), "note"))
    .collect();

  for transcript in transcripts {
    for segment in split_transcript(transcript) {
      candidates.push((segment, "voice"));
    }
  }

  if candidates.is_empty() && !title.trim().is_empty() {
    candidates.push((title.to_string(), "note"));
  }

  let mut out = Vec::new();
  let mut seen = Vec::new();

  for (line, source) in candidates {
    if out.len() >= max_items {
      break;
    }

    let trimmed = line.trim();
    if trimmed.is_empty() || is_checked_item(trimmed) {
      continue;
    }

    if let Some(rest) = checkbox_content(trimmed) {
      push_candidate(&rest, trimmed, source, &mut out, &mut seen, max_items);
      continue;
    }

    if let Some(rest) = explicit_prefix_content(trimmed) {
      push_candidate(&rest, trimmed, source, &mut out, &mut seen, max_items);
      continue;
    }

    let chars: Vec<char> = trimmed.chars().collect();
    let mut found_phrase = false;
    for phrase in PHRASES {
      let mut search_from = 0usize;
      while let Some(relative) = find_ci(&chars[search_from..], phrase) {
        let after = search_from + relative + phrase.chars().count();
        let fragment = take_until_terminator(&chars, after, 90);
        push_candidate(&fragment, trimmed, source, &mut out, &mut seen, max_items);
        found_phrase = true;
        search_from = after;
        if search_from >= chars.len() || out.len() >= max_items {
          break;
        }
      }
    }
    if found_phrase {
      continue;
    }

    if let Some(fragment) = imperative_content(trimmed) {
      push_candidate(&fragment, trimmed, source, &mut out, &mut seen, max_items);
    }
  }

  out
}

fn split_transcript(transcript: &str) -> Vec<String> {
  let mut segments = Vec::new();
  let mut current = String::new();
  for character in transcript.chars() {
    if matches!(character, '.' | '!' | '?' | '\n') {
      let segment = current.trim().to_string();
      if !segment.is_empty() {
        segments.push(segment);
      }
      current.clear();
    } else {
      current.push(character);
    }
  }
  let segment = current.trim().to_string();
  if !segment.is_empty() {
    segments.push(segment);
  }
  segments
}

fn is_checked_item(line: &str) -> bool {
  let without_bullet = line
    .trim_start()
    .trim_start_matches(['-', '*', '•'])
    .trim_start();
  without_bullet.starts_with("[x]") || without_bullet.starts_with("[X]")
}

fn checkbox_content(line: &str) -> Option<String> {
  let without_bullet = line
    .trim_start()
    .trim_start_matches(['-', '*', '•'])
    .trim_start();
  without_bullet
    .strip_prefix("[ ]")
    .map(|rest| rest.trim().to_string())
}

fn explicit_prefix_content(line: &str) -> Option<String> {
  for prefix in EXPLICIT_PREFIXES {
    let chars: Vec<char> = line.chars().collect();
    let needle: Vec<char> = prefix.chars().collect();
    if chars.len() <= needle.len() {
      continue;
    }
    let matches = chars[..needle.len()]
      .iter()
      .zip(needle.iter())
      .all(|(a, b)| a.eq_ignore_ascii_case(b));
    if !matches {
      continue;
    }
    let mut rest = &chars[needle.len()..];
    while rest
      .first()
      .is_some_and(|c| matches!(c, ':' | '-' | '—' | ' ' | '\t'))
    {
      rest = &rest[1..];
    }
    let content: String = rest.iter().collect();
    if !content.trim().is_empty() {
      return Some(content.trim().to_string());
    }
  }
  None
}

fn imperative_content(line: &str) -> Option<String> {
  let stripped = line
    .trim_start()
    .trim_start_matches(['-', '*', '•', ' '])
    .trim_start();
  let lower = normalize(stripped);
  for imperative in IMPERATIVES {
    if lower.starts_with(imperative) {
      let rest: &str = stripped
        .char_indices()
        .nth(imperative.chars().count())
        .map(|(index, _)| &stripped[index..])
        .unwrap_or("");
      let content = format!("{imperative}{rest}");
      return Some(content.trim().to_string());
    }
  }
  None
}

fn take_until_terminator(chars: &[char], start: usize, max_chars: usize) -> String {
  let mut out = String::new();
  for character in chars.iter().skip(start).take(max_chars) {
    if matches!(character, '.' | ';' | '!' | '?' | '\n') {
      break;
    }
    out.push(*character);
  }
  out.trim().to_string()
}

fn push_candidate(
  raw: &str,
  context: &str,
  source: &str,
  out: &mut Vec<ActionItem>,
  seen: &mut Vec<String>,
  max_items: usize,
) {
  if out.len() >= max_items {
    return;
  }

  let cleaned = raw
    .trim()
    .trim_start_matches(['-', '*', '•', '[', ']', ' '])
    .trim_end_matches(['.', ',', ';', ' '])
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ");

  let length = cleaned.chars().count();
  if !(5..=95).contains(&length) {
    return;
  }

  let mut chars = cleaned.chars();
  let Some(first) = chars.next() else {
    return;
  };
  let mut formatted: String = first.to_uppercase().collect();
  formatted.push_str(chars.as_str());

  let key = normalize(&formatted);
  if seen.contains(&key) {
    return;
  }
  seen.push(key);

  // The due hint usually lives in the sentence around the action
  // ("tomorrow I need to call Ana"), not in the extracted fragment itself.
  out.push(ActionItem {
    due_in_minutes: infer_due_offset_minutes(context)
      .or_else(|| infer_due_offset_minutes(&formatted)),
    title: formatted,
    source: source.to_string(),
  });
}

/// Infers a relative due time from phrasing such as "en 2 horas" or "mañana".
pub fn infer_due_offset_minutes(text: &str) -> Option<i64> {
  let normalized = normalize(text);
  let chars: Vec<char> = normalized.chars().collect();

  for preposition in ["en ", "in "] {
    let mut index = 0usize;
    while index < chars.len() {
      let Some(relative) = find_ci(&chars[index..], preposition) else {
        break;
      };
      let after_preposition = index + relative + preposition.chars().count();
      if let Some((value, end)) = parse_number(&chars, after_preposition) {
        let mut unit_start = end;
        while unit_start < chars.len() && chars[unit_start] == ' ' {
          unit_start += 1;
        }
        let unit: String = chars[unit_start..].iter().take(10).collect();
        if unit.starts_with("hora") || unit.starts_with("hours") || unit.starts_with("hour") {
          if (1..=48).contains(&value) {
            return Some(value * 60);
          }
        }
        if unit.starts_with("minuto") || unit.starts_with("minutes") || unit.starts_with("minute") {
          if (1..=1440).contains(&value) {
            return Some(value);
          }
        }
      }
      index = after_preposition;
    }
  }

  if normalized.contains("manana") || normalized.contains("tomorrow") {
    return Some(24 * 60);
  }
  if normalized.contains("esta tarde")
    || normalized.contains("esta noche")
    || normalized.contains("tonight")
  {
    return Some(4 * 60);
  }
  if normalized.contains("hoy") || normalized.contains("today") {
    return Some(2 * 60);
  }
  None
}

fn parse_number(chars: &[char], start: usize) -> Option<(i64, usize)> {
  let mut end = start;
  while end < chars.len() && chars[end].is_ascii_digit() {
    end += 1;
  }
  if end == start {
    return None;
  }
  let digits: String = chars[start..end].iter().collect();
  digits.parse::<i64>().ok().map(|value| (value, end))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extracts_checkbox_items() {
    let items = extract_action_items(
      "Plan",
      "- [ ] Book the flight\n- [x] Already done\nSome prose",
      &[],
      5,
    );
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Book the flight");
    assert_eq!(items[0].source, "note");
  }

  #[test]
  fn extracts_explicit_prefixes_in_spanish() {
    let items = extract_action_items(
      "",
      "Pendiente: pagar la factura de luz\nTAREA: revisar el contrato",
      &[],
      5,
    );
    assert!(
      items
        .iter()
        .any(|item| item.title.contains("Pagar la factura"))
    );
    assert!(
      items
        .iter()
        .any(|item| item.title.contains("Revisar el contrato"))
    );
  }

  #[test]
  fn extracts_natural_language_and_due_time() {
    let items = extract_action_items(
      "",
      "Mañana tengo que llamar a Ana para confirmar la reserva",
      &[],
      5,
    );
    assert_eq!(items.len(), 1);
    assert!(items[0].title.contains("Llamar a Ana"));
    assert_eq!(items[0].due_in_minutes, Some(24 * 60));
  }

  #[test]
  fn extracts_from_voice_transcripts() {
    let items = extract_action_items(
      "",
      "",
      &["Necesito enviar el informe. Luego descanso.".into()],
      5,
    );
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].source, "voice");
  }

  #[test]
  fn infers_relative_offsets() {
    assert_eq!(infer_due_offset_minutes("call in 3 hours"), Some(180));
    assert_eq!(infer_due_offset_minutes("revisar en 30 minutos"), Some(30));
    assert_eq!(infer_due_offset_minutes("pay invoice today"), Some(120));
    assert_eq!(infer_due_offset_minutes("someday maybe"), None);
  }

  #[test]
  fn deduplicates_and_limits() {
    let items = extract_action_items(
      "",
      "TODO: send the report\nTODO: send the report\nTODO: call the bank",
      &[],
      1,
    );
    assert_eq!(items.len(), 1);
  }
}
