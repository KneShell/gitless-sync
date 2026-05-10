//! Pass 2 + Pass 3 of `assemble_entries`. `extract_commit_paths` filters
//! pre-entries that need a Commits API call (Hashed + sha differ); skipped
//! paths follow the G-003 contract. `finalize_entries` runs `classify`
//! against the commit map + `compare` against the normalize-equal map and
//! emits the final [`FileEntry`] vec.
//!
//! Phase 8 task I split — sub-modules per `spec-architecture.md` § Module
//! 폴더 단위 정책. Both leaves depend only on `super::hash_pass` types so
//! `cargo xtask check-cycles` stays acyclic.

mod extract;
mod pre_entry;

pub(super) use extract::extract_commit_paths;
pub(super) use pre_entry::finalize_entries;
