//! Hindsight core: local-first log ingestion, storage, and query.
//!
//! See `docs/ARCHITECTURE.md` for the design. This crate is a scaffold; the
//! modules below mark the intended shape of the code, not a finished API.

/// Parse raw log streams into normalized events.
pub mod ingest {
    //! Format detection and parsers: json, nginx, syslog, k8s, plaintext.
}

/// Embedded DuckDB store for parsed events.
pub mod store {
    //! Owns the DuckDB connection and the `log_events` schema.
}

/// Read-only SQL execution with row/byte caps and a statement timeout.
pub mod query {
    //! Enforces `SELECT`-only access so agent-written SQL stays safe.
}

/// Resolve the local `.hindsight` data directory and load `config.toml`.
pub mod config {
    //! `HINDSIGHT_HOME`, then project-local `./.hindsight`, then `~/.hindsight`.
}
