//! Embedded persistence: `redb`.
//!
//! The backend stores the self-describing record produced by
//! [`crate::codec::encode_symbol`], keyed by the symbol id.
//!
//! The polyglot declarations — the records that can name a `struct` — live in a *second* table
//! beside that one, written by [`crate::codec::encode_universal`], and are read back through
//! `load_redb_universal`. Two tables rather than one, because the two record shapes answer
//! different questions and a reader that had to tell them apart from the blob would eventually
//! guess wrong.

pub mod redb_store;

pub use redb_store::{
    load_redb, load_redb_from, load_redb_universal, load_redb_universal_from, save_redb,
    save_redb_into,
};
