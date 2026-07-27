# Hindsight architecture

Three decisions drive everything below.

1. **Agent-first.** The primary interface is a tool surface an AI agent connects to, not a chat box or a dashboard. An agent discovers what logs exist, reads their shape, and runs queries through a few well-defined calls. Every other choice serves this one.
2. **Rust core.** Ingestion, storage, and query live in a Rust workspace. It ships as a single binary with no runtime or interpreter.
3. **Local.** Hindsight runs where your logs already are and queries them in place, so raw logs stay inside your trust boundary. The same binary is cloud-native: run it as a service in your own environment. This is a trust and cost property, not an offline one. An agent reads every line of your logs, and none of it has to leave your perimeter for it to do that.

## Workspace layout

```
crates/
  hindsight-core     library: ingest (+ follow) + store + query intent + stream
  hindsight-mcp      binary:  MCP server over stdio, for local agents
  hindsight-server   binary:  long-running service: HTTP/SSE MCP, WebSocket live tail, query API
  hindsight-cli      binary `hindsight`: human commands + ingestion
```

`hindsight-core` holds all the logic. The binaries are thin surfaces over it: MCP over stdio for a local agent, a long-running server that exposes the same core over HTTP and WebSocket, and a CLI for the terminal. All of them call the same model-free core. The Portal is a separate application ([its own repo](https://github.com/nooscraft/hindsight-portal), Python/FastAPI) that talks to `hindsight-server`.

## Running Hindsight and where data lives

Hindsight runs as a single binary, either on a machine sitting next to your logs or as a long-running service in your own environment. Either way, data lives under a single directory, resolved in this order:

1. `HINDSIGHT_HOME` if set
2. a project-local `./.hindsight/` found by walking up from the current directory
3. otherwise `~/.hindsight/`

Inside:

```
.hindsight/
  config.toml        sources, parser settings, limits
  hindsight.duckdb   the store
  state/             per-source ingest offsets
```

`config.toml` registers sources. A source is a named log stream with a parser:

```toml
[[source]]
name   = "api"
path   = "/var/log/api/*.log"
format = "json"      # json | nginx | syslog | k8s | auto

[[source]]
name   = "nginx"
path   = "/var/log/nginx/access.log"
format = "nginx"
```

`hindsight ingest` reads registered sources, parses each line, and writes normalized events into DuckDB. Re-running is incremental: each source tracks a byte offset and inode in `state/`, so only new lines get read.

## Data model

One wide event table, plus a sources table.

```
log_events(
  id          BIGINT,
  source      VARCHAR,     -- source name
  ts          TIMESTAMP,   -- parsed event time, null if none found
  level       VARCHAR,     -- normalized: error/warn/info/debug/trace, nullable
  message     VARCHAR,     -- best-effort human-readable message
  fields      JSON,        -- everything else the parser pulled out
  raw         VARCHAR,     -- the original line, always kept
  ingested_at TIMESTAMP
)
```

Keeping `raw` means no parse is lossy. `fields` is queryable with DuckDB's JSON functions, so structured logs keep their structure without a fixed per-source schema.

## Ingestion

A parser turns a raw line into an event. v0 targets:

- `json` — one object per line
- `nginx` — combined access log format
- `syslog` — RFC 5424 and RFC 3164
- `k8s` — JSON logs plus the CRI text format
- plaintext fallback — keeps the line as `raw`, best-effort extraction of a timestamp and level

`format = "auto"` sniffs the first N lines and picks a parser.

### When ingestion happens

Ingestion is decoupled from querying. A query never triggers a read of the source files; it only reads what's already in the store. That keeps queries fast and read-only, and it means freshness depends on how the store is being fed:

- **On demand (batch).** `hindsight ingest` reads new lines from registered sources and stops. The store is as fresh as the last run. Good for one-off analysis or a cron.
- **Continuous (follow).** `hindsight-server` (or `hindsight tail`) runs a follow worker that ingests as lines arrive, so the store stays near real time and live subscribers see events immediately.

Both use the same incremental offsets, so switching between them doesn't re-read or duplicate anything. If you need a query to reflect the absolute latest and you're in batch mode, run `hindsight ingest` first; in follow mode it's already current.

## The query intent

Agents don't send SQL. They send a structured **query intent** against a closed vocabulary, and Hindsight compiles it. This keeps the physical schema and the SQL dialect out of the agent's hands. Swap DuckDB for something else later and no agent breaks.

An intent for "which clients got the most 500s from nginx yesterday":

```json
{
  "source": "nginx",
  "metric": { "count": "*", "as": "errors" },
  "where": [{ "field": "status", "op": "gte", "value": 500 }],
  "group_by": ["client_ip"],
  "order_by": { "field": "errors", "dir": "desc" },
  "time": "yesterday",
  "limit": 4
}
```

The vocabulary:

- `source` — one of the registered sources
- `select` / `metric` — fields to return and aggregates (`count`, `sum`, `avg`, `min`, `max`)
- `where` — a list of `{ field, op, value }`; `op` is a fixed set (`eq`, `ne`, `gt`, `gte`, `lt`, `lte`, `in`, `contains`, `exists`)
- `group_by` — field names
- `order_by` — `{ field, dir }`
- `time` — a named range (`today`, `yesterday`, `last_1h`, `last_24h`, `last_7d`) or explicit `{ since, until }`
- `limit` — capped by the server

A `field` is a column from the data model or a key inside `fields`, which the agent learned from `describe()`.

### Why this stays accurate

The intent is the only place natural language turns into structure, and it's small and closed, so Hindsight checks it before running anything.

- **Validation.** `source` must exist. Every `field` must appear in `describe()`. Operators, aggregates, and named time ranges are enums. Anything unresolved is rejected with the valid options (`unknown field 'stattus'; available: status, client_ip, request_time`) and the agent retries. A hallucinated field never reaches the database.
- **Deterministic compilation.** intent → SQL is code with a fixed mapping and a regression suite, so there's no per-query variance. Hindsight resolves the time range itself, which means the "yesterday accidentally includes today" class of bug can't happen.
- **Echo-back.** Every response restates what actually ran in plain language ("counted nginx rows where status >= 500, grouped by client_ip, 2026-07-26 00:00 to 2026-07-27 00:00") so the agent and the user can confirm the reading.

What this does not guarantee: that the agent's reading matches what the human meant. That ambiguity is irreducible with natural-language input. The contract makes it visible and correctable instead of burying it inside a SQL string.

### Safety

Read-only is now a property of the compiler, not something enforced by rejecting bad SQL. The compiler only ever emits a `SELECT`, so there's no DDL, DML, `ATTACH`, `COPY`, or file-function surface in the first place. On top of that the executor:

- runs against DuckDB with a statement timeout
- caps result rows and total bytes, and reports truncation in the response

Because the agent never sends raw SQL, there's nothing to sanitize. `where` values are bound as parameters, not interpolated into a string.

## The agent surface

`hindsight-mcp` exposes tools over MCP, stdio to start and HTTP/SSE later. The goal is that an agent can assess a system's logs with minimal ceremony.

- `list_sources()` — each source with row count and time range
- `describe(source?)` — columns, detected `fields` keys with inferred types, and a few sample values per field
- `query(intent)` — run a validated query intent; returns rows, an echo of what ran, and a `truncated` flag
- `stats(source, field)` — cardinality and top values for a field
- `sample(source, n)` — n recent rows

Normal agent loop: `list_sources` to see what's here, `describe` to learn the shape, then build a query intent and call `query`. The agent works in domain terms, sources and field names, and never touches SQL or the physical schema.

## Live streaming

Logs matter most while something is happening: a deploy, an incident, a request that's stuck. Hindsight follows sources as they're written and pushes new events to whoever is watching.

- **Follow.** The ingest layer can tail a source, watching for new lines by offset and inode, and parse each one through the same path as batch ingest. Nothing about the parser changes; it's the trigger that's different.
- **Fan-out.** A follow worker does two things with each new event. It batch-writes it to DuckDB for history, and publishes it immediately to an in-process broadcast bus for live viewers. The live path is fed by the bus, so it doesn't wait on the database. DuckDB gets writes in small batches (every N lines or N milliseconds) because it's built for analytical queries, not a write per line.
- **Subscriptions.** A live subscriber sends a filter, the same vocabulary as a query intent minus the aggregation (`source`, `where`, `level`, text match), and gets matching events pushed as they arrive. The Portal uses this for a live tail. An agent can use it too, to watch for a condition rather than poll.

## The Portal

The CLI serves terminals and MCP serves agents. The Portal is the human window into a running Hindsight: a live tail, an audit of what agents did, and a look at how healthy ingestion is.

It lives in its own repo and is built with Python and FastAPI, separate from the Rust core: https://github.com/nooscraft/hindsight-portal

It covers the things a person still wants their own eyes on:

- a **live tail** with filters (source, level, text), backed by the WebSocket subscriptions
- the **echo-backs**: what agents actually ran, so a human can see and trust the query behind an answer
- **source and ingest health**: what's registered, row counts, time ranges, how far behind a follow is
- an **ad-hoc view** for poking at something without an agent in the loop

It's deliberately not part of the core. The Rust side exposes an HTTP and WebSocket API through `hindsight-server`, and the Portal is a client of that API. It holds no logs, runs no model, and reuses the query intent contract and the live subscriptions, so it stays a thin surface rather than a second implementation. Python/FastAPI lets the UI move at its own pace without touching the Rust core, and it keeps the agent-first story intact: the Portal is where a human watches live logs and audits what agents did, not a way back to grepping history by hand.

## Trust and auth boundary

Hindsight has three kinds of caller, and they don't get the same trust.

- **Local stdio (`hindsight-mcp`, `hindsight-cli`).** These run as a user on the same machine as the store. Whoever can execute the binary already has the logs, so there's nothing to gate. No tokens.
- **Network (`hindsight-server`).** Every request is authenticated. Read-only-by-construction still holds, but on a network you also need to know who's asking. The open-source server ships simple token auth: tokens live in config or the environment and can be scoped to specific sources.
- **The Portal.** Just another authenticated client of `hindsight-server`. It holds no special privilege.

The core validates identity; it does not manage users. That split is deliberate, and it's where the optional control plane plugs in. Raw logs stay in the data plane no matter what.

```
  control plane (optional, hosted)          data plane (always the customer's)
  ┌────────────────────────────┐            ┌───────────────────────────────┐
  │ identity, orgs, RBAC, SSO  │  config →  │  hindsight-server + DuckDB    │
  │ fleet view, audit, billing │  ← health, │  raw logs, never leave here   │
  └────────────────────────────┘   metadata └───────────────────────────────┘
                                    results
```

- **Self-hosted (open source):** you set tokens yourself. No control plane, no third party.
- **Managed control plane (paid, optional):** it issues and rotates tokens, maps them to orgs and roles, and the server validates them (a signed token it can check on its own, like a JWT). Enrollment is one-time: a server registers with an enrollment token, gets a signed identity, and then shows up in the fleet.

What crosses the boundary is fixed: policy and config down, health and metadata and explicitly-requested query results up. The control plane can say "this user may query the nginx source," but it never receives the nginx logs. See [MONETIZATION.md](MONETIZATION.md) for how this boundary maps to the business model.

## Where the natural language lives

An LLM is always in the loop, because something has to understand English. The design keeps it on the caller's side. Hindsight itself runs no model.

1. **Agent-first (primary).** An external agent (Grok, Claude, whatever the user runs) reads the user's English and emits query intents to `hindsight-mcp`. It's the only LLM involved. Hindsight validates, compiles, executes, and echoes back. It ships no model and holds no API key.
2. **Built-in ask (optional).** `hindsight ask "..."` uses your API key to run an in-process agent that emits the same intents, for people who just want a CLI. The core is unchanged and still model-free.

Either way, Hindsight is a deterministic validator, compiler, and executor. The English → intent step is the model's job, on the caller's side.

## Roadmap

**v0**

- workspace, config, and data-dir resolution
- json / nginx / syslog / plaintext parsers with incremental ingest and follow mode
- DuckDB store and the intent compiler (validation, deterministic compilation, read-only execution with caps)
- MCP server (stdio) with `list_sources` / `describe` / `query(intent)` / `stats`
- in-process live tail via the broadcast bus, surfaced by `hindsight tail`
- `hindsight ingest`, `hindsight query`, `hindsight tail`

**v1**

- `hindsight-server`: HTTP/SSE MCP transport, WebSocket live tail, query API
- the Portal ([separate repo](https://github.com/nooscraft/hindsight-portal), Python/FastAPI): live tail view, echo-back audit, source and ingest health, ad-hoc views
- k8s parser with label extraction
- `hindsight ask` built-in NL
- saved queries and named views
