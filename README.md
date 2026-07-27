# Hindsight

Natural-language log analyzer. Local-first, agent-first: point an AI agent at your logs and let it discover, inspect, and query them without anything leaving your machine.

Hindsight parses logs in any format (JSON, nginx, syslog, k8s) into an embedded DuckDB store on your machine. An AI agent connects over MCP and gets a small set of read-only tools to assess what happened. No cloud, no data egress, no model shipped inside.

## Design

- **Rust core.** Ingestion, storage, and query are a Rust workspace that ships as a single binary.
- **Local-first.** Everything runs and stays on your machine. The store is an embedded DuckDB file.
- **Agent-first.** The primary interface is a tool surface for AI agents, not a dashboard. An agent discovers sources, reads their shape, and runs read-only queries.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full design.

## Workspace

- `crates/hindsight-core` — ingestion, storage, query engine
- `crates/hindsight-mcp` — MCP server, the agent-facing surface
- `crates/hindsight-cli` — the `hindsight` binary for humans

## Status

Early development. The repo is a scaffold; see the roadmap in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## License

MIT
