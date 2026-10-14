//! The storage suite: the canonical tables, and the schema handshake that decides whether a
//! workspace database is readable or has to be rebuilt.
//!
//! ```text
//! cargo test -p storage
//! cargo test -p storage --test persistence tables
//! cargo test -p storage --test persistence recovery
//! ```

mod recovery;
mod references;
mod support;
mod tables;
mod workspace;
