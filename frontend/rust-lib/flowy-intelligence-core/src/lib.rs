//! Deterministic local text intelligence.
//!
//! Everything in this crate runs offline with plain `std`, so the features it
//! powers (summaries, action items, titles, keywords, entities, insights and
//! explainable search) work without an account, an API key or a model
//! download. The algorithms are ported from Mnemosyne's on-device semantic
//! engine and adapted to AppFlowy's documents.

pub mod actions;
pub mod entities;
pub mod insights;
pub mod keywords;
pub mod search;
pub mod summary;
pub mod text;
pub mod title;
