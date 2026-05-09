//! `.gitattributes` parser — working tree only.
//!
//! Three responsibilities split across three sub-modules so each fits
//! within the Phase 6 LOC budget without sibling test files (spec
//! `spec-architecture.md` § 금지 패턴):
//!
//! - [`parser`] — line tokenization (`RawAttribute`, `LineRule`).
//! - [`classify`] — whitelist mapping (`AttributeMatch`).
//! - [`matching`] — working-tree discovery + per-path lookup
//!   (`GitAttributes`).
//!
//! K1.5 layers `AttributeMatch` + [`GitAttributes::classify_path`] on top of
//! the raw match list to reduce a path's tokens to one whitelist bucket; K2
//! consumes that bucket from `prepare_for_hash` (`shared::normalize`). See
//! `docs/specs/spec-hash-and-normalize.md` § `.gitattributes` 파서 and
//! `docs/specs/spec-config.md` § 위치 정책.
//!
//! Sub-module sibling cross-refs (`use super::parser::Rule;`) are allowed
//! per `spec-architecture.md` § Module 폴더 단위 정책 — module folders are
//! a single-responsibility cluster, not a vertical slice.
#![allow(dead_code)]

mod classify;
mod matching;
mod parser;

pub use classify::AttributeMatch;
pub use matching::GitAttributes;
pub use parser::RawAttribute;
