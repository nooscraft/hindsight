//! Hindsight server: the long-running service.
//!
//! Exposes the core over the network: a WebSocket live-tail endpoint, the
//! HTTP/SSE MCP transport, and the query API. This is the API the Portal
//! (a separate Python/FastAPI app) and remote agents consume. Scaffold only.

fn main() {
    eprintln!("hindsight-server: not yet implemented. See docs/ARCHITECTURE.md");
}
