//! sq-core: the frozen contract shared by every sparqlite crate.
//! No SQLite dependency. Changes after day 0 are additive-only and need lead review.
#![forbid(unsafe_code)]
#![allow(rustdoc::broken_intra_doc_links, rustdoc::invalid_html_tags)]

pub mod batch;
pub mod codec;
pub mod error;
pub mod eval_api;
pub mod id;
pub mod ir;
pub mod plan;
pub mod stats;
pub mod store;
pub mod term;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use batch::{Batch, Slot, SlotSet, BATCH_ROWS};
pub use error::{Error, Result};
pub use id::{Kind, Tag, TermId};
pub use term::{Literal, Term};

pub use oxrdf;
pub use spargebra;
pub use rustc_hash::{FxHashMap, FxHashSet};

#[cfg(test)]
mod tests;
