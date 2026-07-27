//! Hindsight core: local-first log ingestion, storage, and query.
//!
//! See `docs/ARCHITECTURE.md` for the design. This crate is a scaffold; the
//! modules below mark the intended shape of the code, not a finished API.

/// Parse raw log streams into normalized events.
pub mod ingest {
    //! Format detection and parsers: json, nginx, syslog, k8s, plaintext.
    //! Runs on demand (`hindsight ingest`) or continuously (follow mode).
}

/// Live event fan-out for follow mode.
pub mod stream {
    //! Follow workers publish parsed events to a broadcast bus. Live viewers
    //! (the Portal, or an agent) subscribe with a filter and receive matching
    //! events as they arrive, without waiting on the store.
}

/// Embedded DuckDB store for parsed events.
pub mod store {
    //! Owns the DuckDB connection and the `log_events` schema.
}

/// Validate query intents, compile them to SQL, and run them read-only.
pub mod query {
    //! Agents send a structured query intent, not SQL. This module validates
    //! it against the live schema, compiles it to a `SELECT` (read-only by
    //! construction), and executes it with row/byte caps and a timeout.
}

/// Resolve the local `.hindsight` data directory and load `config.toml`.
pub mod config {
    //! `HINDSIGHT_HOME`, then project-local `./.hindsight`, then `~/.hindsight`.
}
