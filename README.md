# Hindsight

Agent-first log analysis. Hindsight gives AI agents a small, read-only tool surface to discover, inspect, and query your logs, so an agent can assess what happened without a human ever opening a log file.

It parses logs in any format (JSON, nginx, syslog, k8s) into a DuckDB store, follows them live, and runs wherever your logs already live: a host, a container, your cluster, or as a hosted service in your own environment. Cloud-native, with no forced egress of raw logs. A Portal gives a human a live tail and an audit of what agents actually ran.

## Design

- **Agent-first.** The primary interface is an MCP tool surface for AI agents, not a dashboard. An agent discovers sources, reads their shape, and runs read-only queries. Every other choice serves this one.
- **Rust core.** Ingestion, storage, and query are a Rust workspace that ships as a single binary.
- **Local.** Hindsight runs next to your logs and queries them in place, so raw logs stay inside your trust boundary instead of being shipped to a third party. It's cloud-native too: deploy the same binary as a service in your own environment. The point isn't running offline, it's that your logs (and the secrets that leak into them) don't leave your perimeter just because an agent is reading them.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full design.

## Workspace

- `crates/hindsight-core` — ingestion (with follow/live tail), storage, query intent engine
- `crates/hindsight-mcp` — MCP server over stdio, for local agents
- `crates/hindsight-server` — long-running service: HTTP MCP, WebSocket live tail, query API
- `crates/hindsight-cli` — the `hindsight` binary for humans

The **Portal** (web UI) lives in a separate repo, built with Python/FastAPI: [nooscraft/hindsight-portal](https://github.com/nooscraft/hindsight-portal). It talks to `hindsight-server` over HTTP and WebSocket.

## Status

Early development. The repo is a scaffold; see the roadmap in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## License

MIT
