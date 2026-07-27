# Portal

The web UI for a running Hindsight, served by `hindsight-server`.

The Portal is the human window into the system: a live tail with filters, an audit of what agents actually ran (the echo-backs), source and ingest health, and an ad-hoc view for poking at logs without an agent in the loop.

It talks to the same core over HTTP and WebSocket, runs no model of its own, and reuses the query intent contract and the live subscriptions. See [../docs/ARCHITECTURE.md](../docs/ARCHITECTURE.md) for the design.

## Status

Planned for v1. Frontend stack not chosen yet.
