//! Local entity discovery and knowledge graph construction.
//!
//! The engine finds the things a document talks about (people, places,
//! organizations, projects, topics, links and dates) using deterministic,
//! explainable rules, then links documents and entities into a graph. No
//! external NLP service is involved, so it works for every user offline.

use crate::text::{is_stopword, normalize, slug};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EntityKind {
  Person,
  Place,
  Organization,
  Project,
  Topic,
  Url,
  Email,
  DateTime,
}

impl EntityKind {
  pub fn as_str(self) -> &'static str {
    match self {
      EntityKind::Person => "person",
      EntityKind::Place => "place",
      EntityKind::Organization => "organization",
      EntityKind::Project => "project",
      EntityKind::Topic => "topic",
      EntityKind::Url => "url",
      EntityKind::Email => "email",
      EntityKind::DateTime => "datetime",
    }
  }

  /// Stable prefix so a document node and an entity node can never collide.
  pub fn node_id(self, normalized: &str) -> String {
    format!("{}:{}", self.as_str(), slug(normalized))
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mention {
  pub raw: String,
  /// Byte offset in the source document, ready for highlighting in the editor.
  pub start: usize,
  pub end: usize,
  pub kind: EntityKind,
  pub normalized: String,
  pub confidence: f32,
}

const KNOWN_ORGANIZATIONS: &[&str] = &[
  "google",
  "apple",
  "microsoft",
  "github",
  "amazon",
  "meta",
  "openai",
  "anthropic",
  "notion",
  "figma",
  "slack",
  "stripe",
  "spotify",
  "netflix",
  "mozilla",
  "mit",
  "stanford",
  "harvard",
  "university",
  "appflowy",
];

const KNOWN_PLACES: &[&str] = &[
  "paris",
  "london",
  "tokyo",
  "berlin",
  "madrid",
  "barcelona",
  "mexico",
  "bogota",
  "lima",
  "buenos",
  "aires",
  "santiago",
  "miami",
  "orlando",
  "seattle",
  "portland",
  "kyoto",
  "new",
  "york",
  "san",
  "francisco",
  "beach",
  "park",
  "airport",
  "station",
  "museum",
  "gym",
  "office",
  "cabin",
  "lake",
  "trail",
  "hotel",
  "cafe",
  "library",
  "studio",
];

const MONTHS: &[&str] = &[
  "january",
  "february",
  "march",
  "april",
  "may",
  "june",
  "july",
  "august",
  "september",
  "october",
  "november",
  "december",
  "enero",
  "febrero",
  "marzo",
  "abril",
  "mayo",
  "junio",
  "julio",
  "agosto",
  "septiembre",
  "setiembre",
  "octubre",
  "noviembre",
  "diciembre",
];

const PROJECT_PREFIXES: &[&str] = &["project", "initiative", "proyecto", "iniciativa"];

/// Extracts every entity mention from `text`, ordered by position.
pub fn extract_mentions(text: &str) -> Vec<Mention> {
  let chars: Vec<char> = text.chars().collect();
  let mut byte_offsets = Vec::with_capacity(chars.len() + 1);
  let mut offset = 0usize;
  for character in &chars {
    byte_offsets.push(offset);
    offset += character.len_utf8();
  }
  byte_offsets.push(offset);

  let mut out = MentionCollector::default();
  extract_urls(&chars, &byte_offsets, &mut out);

  let words = split_words(&chars);
  let mut index = 0usize;
  while index < words.len() {
    let word = &words[index];

    if is_email(&word.text) {
      out.push(
        word.text.clone(),
        word.start,
        word.end,
        EntityKind::Email,
        0.99,
        &byte_offsets,
      );
      index += 1;
      continue;
    }

    if is_iso_date(&word.text) {
      out.push(
        word.text.clone(),
        word.start,
        word.end,
        EntityKind::DateTime,
        0.95,
        &byte_offsets,
      );
      index += 1;
      continue;
    }

    if is_month(&word.text) {
      // "January 5" / "5 de enero" style dates.
      let mut end = index;
      if index + 1 < words.len() && is_day_number(&words[index + 1].text) {
        end = index + 1;
      }
      let raw = join_words(&chars, &words, index, end);
      out.push(
        raw,
        word.start,
        words[end].end,
        EntityKind::DateTime,
        0.9,
        &byte_offsets,
      );
      index = end + 1;
      continue;
    }

    if word.text.starts_with('#') && word.text.chars().count() > 1 {
      let label = word.text.trim_start_matches('#');
      out.push(
        label.to_string(),
        word.start,
        word.end,
        EntityKind::Topic,
        0.9,
        &byte_offsets,
      );
      index += 1;
      continue;
    }

    if is_capitalized(&word.text) && !is_stopword(&normalize(&word.text)) {
      let mut end = index;
      while end + 1 < words.len()
        && end - index < 2
        && is_capitalized(&words[end + 1].text)
        && !is_stopword(&normalize(&words[end + 1].text))
        && adjacent(&chars, &words[end], &words[end + 1])
      {
        end += 1;
      }

      let (kind, confidence) =
        classify_capitalized(&words[index..=end], is_sentence_start(&chars, word.start));
      if let Some(kind) = kind {
        let raw = join_words(&chars, &words, index, end);
        out.push(
          raw,
          word.start,
          words[end].end,
          kind,
          confidence,
          &byte_offsets,
        );
      }
      index = end + 1;
      continue;
    }

    index += 1;
  }

  out.finish()
}

#[derive(Default)]
struct MentionCollector {
  mentions: Vec<Mention>,
  seen: HashMap<(EntityKind, String), usize>,
}

impl MentionCollector {
  fn push(
    &mut self,
    raw: String,
    start_char: usize,
    end_char: usize,
    kind: EntityKind,
    confidence: f32,
    byte_offsets: &[usize],
  ) {
    let raw = raw.trim().to_string();
    let normalized = normalize(&raw);
    if normalized.is_empty() {
      return;
    }

    let key = (kind, normalized.clone());
    if let Some(&existing) = self.seen.get(&key) {
      let mention: &mut Mention = &mut self.mentions[existing];
      mention.confidence = mention.confidence.max(confidence);
      return;
    }

    self.seen.insert(key, self.mentions.len());
    self.mentions.push(Mention {
      start: byte_offsets[start_char.min(byte_offsets.len() - 1)],
      end: byte_offsets[end_char.min(byte_offsets.len() - 1)],
      raw,
      kind,
      normalized,
      confidence,
    });
  }

  fn finish(mut self) -> Vec<Mention> {
    self.mentions.sort_by(|a, b| a.start.cmp(&b.start));
    self.mentions
  }
}

#[derive(Debug, Clone)]
struct Word {
  text: String,
  /// Character indices (not bytes).
  start: usize,
  end: usize,
}

fn split_words(chars: &[char]) -> Vec<Word> {
  let mut words = Vec::new();
  let mut index = 0usize;
  while index < chars.len() {
    let character = chars[index];
    if character.is_alphanumeric() || character == '#' || character == '@' {
      let start = index;
      index += 1;
      while index < chars.len()
        && (chars[index].is_alphanumeric()
          || matches!(chars[index], '#' | '@' | '.' | '_' | '-' | '/' | ':'))
      {
        // A trailing '.' or ',' does not belong to the word.
        if matches!(chars[index], '.' | ':' | '/')
          && chars
            .get(index + 1)
            .is_none_or(|next| !next.is_alphanumeric())
        {
          break;
        }
        index += 1;
      }
      words.push(Word {
        text: chars[start..index].iter().collect(),
        start,
        end: index,
      });
    } else {
      index += 1;
    }
  }
  words
}

fn adjacent(chars: &[char], left: &Word, right: &Word) -> bool {
  chars[left.end..right.start]
    .iter()
    .all(|c| c.is_whitespace())
}

fn is_sentence_start(chars: &[char], start: usize) -> bool {
  chars[..start]
    .iter()
    .rev()
    .find(|c| !c.is_whitespace())
    .is_none_or(|c| matches!(c, '.' | '!' | '?' | '\n' | ':' | ';'))
}

fn join_words(chars: &[char], words: &[Word], start: usize, end: usize) -> String {
  let from = words[start].start;
  let to = words[end].end;
  chars[from..to].iter().collect()
}

fn is_capitalized(word: &str) -> bool {
  word.chars().next().is_some_and(|c| c.is_uppercase())
}

fn is_month(word: &str) -> bool {
  let normalized = normalize(word);
  MONTHS.contains(&normalized.as_str())
}

fn is_day_number(word: &str) -> bool {
  let digits: String = word.chars().filter(|c| c.is_ascii_digit()).collect();
  word.chars().count() <= 2 && !digits.is_empty() && digits.parse::<u32>().is_ok_and(|d| d <= 31)
}

fn is_iso_date(word: &str) -> bool {
  let chars: Vec<char> = word.chars().collect();
  chars.len() == 10
    && chars[..4].iter().all(|c| c.is_ascii_digit())
    && chars[4] == '-'
    && chars[5..7].iter().all(|c| c.is_ascii_digit())
    && chars[7] == '-'
    && chars[8..].iter().all(|c| c.is_ascii_digit())
}

fn is_email(word: &str) -> bool {
  let trimmed = word.trim_matches(|c: char| {
    !c.is_alphanumeric() && c != '@' && c != '.' && c != '_' && c != '-' && c != '+'
  });
  match trimmed.split_once('@') {
    Some((local, domain)) => {
      !local.is_empty()
        && domain.contains('.')
        && domain
          .rsplit('.')
          .next()
          .is_some_and(|suffix| suffix.len() >= 2)
    },
    None => false,
  }
}

fn classify_capitalized(words: &[Word], at_sentence_start: bool) -> (Option<EntityKind>, f32) {
  let first = normalize(&words[0].text);
  let joined = words
    .iter()
    .map(|word| normalize(&word.text))
    .collect::<Vec<_>>()
    .join(" ");

  if PROJECT_PREFIXES.contains(&first.as_str()) && words.len() > 1 {
    return (Some(EntityKind::Project), 0.9);
  }

  if words.len() == 1 {
    if KNOWN_ORGANIZATIONS.contains(&first.as_str()) {
      return (Some(EntityKind::Organization), 0.85);
    }
    if KNOWN_PLACES.contains(&first.as_str()) {
      return (Some(EntityKind::Place), 0.8);
    }
    // A single capitalised word at the start of a sentence is most likely
    // grammar, not a name. Mid-sentence it is usually a person or brand.
    if at_sentence_start {
      return (None, 0.0);
    }
    return (Some(EntityKind::Person), 0.55);
  }

  if words.iter().any(|word| {
    let word = normalize(&word.text);
    KNOWN_ORGANIZATIONS.contains(&word.as_str())
      || [
        "inc",
        "ltd",
        "llc",
        "gmbh",
        "university",
        "lab",
        "labs",
        "studio",
      ]
      .contains(&word.as_str())
  }) {
    return (Some(EntityKind::Organization), 0.8);
  }

  if words
    .iter()
    .any(|word| KNOWN_PLACES.contains(&normalize(&word.text).as_str()))
  {
    return (Some(EntityKind::Place), 0.7);
  }

  if joined.is_empty() {
    return (None, 0.0);
  }

  (Some(EntityKind::Person), 0.72)
}

fn extract_urls(chars: &[char], byte_offsets: &[usize], out: &mut MentionCollector) {
  let mut index = 0usize;
  while index < chars.len() {
    let has_scheme = matches_at(chars, index, "http://") || matches_at(chars, index, "https://");
    if !has_scheme {
      index += 1;
      continue;
    }

    let mut end = index;
    while end < chars.len()
      && !chars[end].is_whitespace()
      && !matches!(chars[end], ')' | ']' | '"' | '\'' | '>' | '»' | '<')
    {
      end += 1;
    }

    // Trailing punctuation is part of the sentence, not of the link.
    while end > index && matches!(chars[end - 1], '.' | ',' | ';' | ':' | '!' | '?') {
      end -= 1;
    }

    let raw: String = chars[index..end].iter().collect();
    out.push(raw, index, end, EntityKind::Url, 0.99, byte_offsets);
    index = end.max(index + 1);
  }
}

fn matches_at(chars: &[char], start: usize, needle: &str) -> bool {
  let needle: Vec<char> = needle.chars().collect();
  if start + needle.len() > chars.len() {
    return false;
  }
  chars[start..start + needle.len()]
    .iter()
    .zip(needle.iter())
    .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

#[derive(Debug, Clone)]
pub struct DocEntities {
  pub document_id: String,
  pub title: String,
  pub mentions: Vec<Mention>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GraphNodeKind {
  Document,
  Entity(EntityKind),
}

#[derive(Debug, Clone)]
pub struct GraphNode {
  pub id: String,
  pub label: String,
  pub kind: GraphNodeKind,
  pub mentions: u32,
}

#[derive(Debug, Clone)]
pub struct GraphEdge {
  pub source: String,
  pub target: String,
  pub weight: u32,
}

#[derive(Debug, Clone, Default)]
pub struct KnowledgeGraph {
  pub nodes: Vec<GraphNode>,
  pub edges: Vec<GraphEdge>,
}

/// Builds the document/entity graph. Document ↔ entity edges carry the mention
/// count; entity ↔ entity edges carry how often the two co-occur in a document.
pub fn build_graph(documents: &[DocEntities]) -> KnowledgeGraph {
  let mut nodes: BTreeMap<String, GraphNode> = BTreeMap::new();
  let mut edges: HashMap<(String, String), u32> = HashMap::new();

  for document in documents {
    let document_node_id = format!("document:{}", document.document_id);
    nodes.entry(document_node_id.clone()).or_insert(GraphNode {
      id: document_node_id.clone(),
      label: document.title.clone(),
      kind: GraphNodeKind::Document,
      mentions: 0,
    });

    let mut per_document: HashMap<(EntityKind, String), u32> = HashMap::new();
    for mention in &document.mentions {
      *per_document
        .entry((mention.kind, mention.normalized.clone()))
        .or_insert(0) += 1;
    }

    let entity_ids: Vec<String> = per_document
      .keys()
      .map(|(kind, normalized)| kind.node_id(normalized))
      .collect();

    for ((kind, normalized), count) in &per_document {
      let node_id = kind.node_id(normalized);
      let node = nodes.entry(node_id.clone()).or_insert_with(|| GraphNode {
        id: node_id.clone(),
        label: normalized.clone(),
        kind: GraphNodeKind::Entity(*kind),
        mentions: 0,
      });
      node.mentions += count;
      increment_edge(&mut edges, &document_node_id, &node_id, *count);
    }

    // Co-occurrence edges only between "semantic" entities; a shared URL says
    // little about how two notes relate.
    let semantic: Vec<&String> = entity_ids
      .iter()
      .filter(|id| {
        !id.starts_with("url:") && !id.starts_with("email:") && !id.starts_with("datetime:")
      })
      .collect();
    for (left, right) in pairs(&semantic) {
      increment_edge(&mut edges, left, right, 1);
    }
  }

  let mut edges = edges
    .into_iter()
    .map(|((source, target), weight)| GraphEdge {
      source,
      target,
      weight,
    })
    .collect::<Vec<_>>();
  edges.sort_by(|a, b| {
    b.weight
      .cmp(&a.weight)
      .then_with(|| a.source.cmp(&b.source))
      .then_with(|| a.target.cmp(&b.target))
  });

  KnowledgeGraph {
    nodes: nodes.into_values().collect(),
    edges,
  }
}

fn increment_edge(edges: &mut HashMap<(String, String), u32>, a: &str, b: &str, weight: u32) {
  let (source, target) = if a <= b { (a, b) } else { (b, a) };
  *edges
    .entry((source.to_string(), target.to_string()))
    .or_insert(0) += weight;
}

fn pairs<'a>(items: &[&'a String]) -> Vec<(&'a String, &'a String)> {
  let mut out = Vec::new();
  for (index, left) in items.iter().enumerate() {
    for right in &items[index + 1..] {
      out.push((*left, *right));
    }
  }
  out
}

/// Entities ordered by how connected they are, excluding documents. This is
/// what an "insights" surface shows: the people, places and topics that keep
/// coming back.
pub fn hubs(graph: &KnowledgeGraph, limit: usize) -> Vec<GraphNode> {
  let mut degree: HashMap<&str, u32> = HashMap::new();
  for edge in &graph.edges {
    *degree.entry(edge.source.as_str()).or_insert(0) += edge.weight;
    *degree.entry(edge.target.as_str()).or_insert(0) += edge.weight;
  }

  let mut entities: Vec<GraphNode> = graph
    .nodes
    .iter()
    .filter(|node| !matches!(node.kind, GraphNodeKind::Document))
    .cloned()
    .collect();

  entities.sort_by(|a, b| {
    degree
      .get(b.id.as_str())
      .copied()
      .unwrap_or(0)
      .cmp(&degree.get(a.id.as_str()).copied().unwrap_or(0))
      .then_with(|| b.mentions.cmp(&a.mentions))
      .then_with(|| a.label.cmp(&b.label))
  });
  entities.truncate(limit);
  entities
}

/// Distinct entities across all documents, most mentioned first.
pub fn top_entities(documents: &[DocEntities], limit: usize) -> Vec<GraphNode> {
  let mut seen: HashMap<(EntityKind, String), (String, u32)> = HashMap::new();
  for document in documents {
    for mention in &document.mentions {
      let entry = seen
        .entry((mention.kind, mention.normalized.clone()))
        .or_insert((mention.raw.clone(), 0));
      entry.1 += 1;
    }
  }

  let mut out: Vec<GraphNode> = seen
    .into_iter()
    .map(|((kind, normalized), (raw, mentions))| GraphNode {
      id: kind.node_id(&normalized),
      label: raw,
      kind: GraphNodeKind::Entity(kind),
      mentions,
    })
    .collect();

  out.sort_by(|a, b| {
    b.mentions
      .cmp(&a.mentions)
      .then_with(|| a.label.cmp(&b.label))
  });
  out.truncate(limit);
  out
}

/// All distinct normalized labels of the given kinds, useful for autolinking.
pub fn labels_of(documents: &[DocEntities], kinds: &[EntityKind]) -> HashSet<String> {
  let kinds: HashSet<EntityKind> = kinds.iter().copied().collect();
  documents
    .iter()
    .flat_map(|document| document.mentions.iter())
    .filter(|mention| kinds.contains(&mention.kind))
    .map(|mention| mention.normalized.clone())
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn kinds(text: &str) -> Vec<EntityKind> {
    extract_mentions(text).into_iter().map(|m| m.kind).collect()
  }

  #[test]
  fn finds_links_and_emails() {
    let mentions = extract_mentions("Ping ana@example.com about https://appflowy.io/docs.");
    assert!(mentions.iter().any(|m| m.kind == EntityKind::Email));
    let url = mentions.iter().find(|m| m.kind == EntityKind::Url).unwrap();
    assert_eq!(url.raw, "https://appflowy.io/docs");
  }

  #[test]
  fn finds_projects_topics_and_dates() {
    assert!(kinds("We shipped Project Aurora today").contains(&EntityKind::Project));
    assert!(kinds("Notes for #roadmap").contains(&EntityKind::Topic));
    assert!(kinds("Due on 2026-01-05").contains(&EntityKind::DateTime));
    assert!(kinds("Meeting on January 5").contains(&EntityKind::DateTime));
  }

  #[test]
  fn does_not_treat_sentence_starts_as_names() {
    let mentions = extract_mentions("Great meeting today. Notes are done.");
    assert!(mentions.iter().all(|m| m.kind != EntityKind::Person));
  }

  #[test]
  fn finds_people_mid_sentence() {
    let mentions = extract_mentions("I met Ana Garcia at the cafe");
    assert!(
      mentions
        .iter()
        .any(|m| m.kind == EntityKind::Person && m.raw == "Ana Garcia")
    );
  }

  #[test]
  fn offsets_point_at_the_original_text() {
    let text = "Email ana@example.com now";
    let mention = extract_mentions(text)
      .into_iter()
      .find(|m| m.kind == EntityKind::Email)
      .unwrap();
    assert_eq!(&text[mention.start..mention.end], "ana@example.com");
  }

  #[test]
  fn builds_a_graph_and_ranks_hubs() {
    let documents = vec![
      DocEntities {
        document_id: "1".into(),
        title: "Trip".into(),
        mentions: extract_mentions("I met Ana Garcia in Kyoto"),
      },
      DocEntities {
        document_id: "2".into(),
        title: "Follow up".into(),
        mentions: extract_mentions("Ana Garcia sent the Kyoto photos"),
      },
    ];
    let graph = build_graph(&documents);
    assert_eq!(
      graph
        .nodes
        .iter()
        .filter(|n| matches!(n.kind, GraphNodeKind::Document))
        .count(),
      2
    );
    let hubs = hubs(&graph, 5);
    assert!(!hubs.is_empty());
    assert!(hubs.iter().any(|hub| hub.label.contains("ana")));
  }

  #[test]
  fn top_entities_counts_mentions() {
    let documents = vec![DocEntities {
      document_id: "1".into(),
      title: "T".into(),
      mentions: extract_mentions("Kyoto and Kyoto again"),
    }];
    let top = top_entities(&documents, 5);
    assert!(top.iter().any(|node| node.label.contains("Kyoto")));
  }
}
