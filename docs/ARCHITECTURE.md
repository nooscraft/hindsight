# Hindsight architecture

Three decisions drive everything below.

1. **Agent-first.** The primary interface is a tool surface an AI agent connects to, not a chat box or a dashboard. An agent discovers what logs exist, reads their shape, and runs queries through a few well-defined calls. Every other choice serves this one.
2. **Rust core.** Ingestion, storage, and query live in a Rust workspace. It ships as a single binary with no runtime or interpreter.
3. **Local.** Hindsight runs where your logs already are and queries them in place, so raw logs stay inside your trust boundary. The same binary is cloud-native: run it as a service in your own environment. This is a trust and cost property, not an offline one. An agent reads every line of your logs, and none of it has to leave your perimeter for it to do that.

## Workspace layout

```
crates/
  hindsight-core   library: ingest + store + query
  hindsight-mcp    binary:  MCP server, the agent surface
  hindsight-cli    binary `hindsight`: human commands + ingestion
```

`hindsight-core` holds all the logic. The two binaries are thin: one speaks MCP to agents, the other speaks to a person at a terminal. Both call the same core.

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

## Where the natural language lives

An LLM is always in the loop, because something has to understand English. The design keeps it on the caller's side. Hindsight itself runs no model.

1. **Agent-first (primary).** An external agent (Grok, Claude, whatever the user runs) reads the user's English and emits query intents to `hindsight-mcp`. It's the only LLM involved. Hindsight validates, compiles, executes, and echoes back. It ships no model and holds no API key.
2. **Built-in ask (optional).** `hindsight ask "..."` uses your API key to run an in-process agent that emits the same intents, for people who just want a CLI. The core is unchanged and still model-free.

Either way, Hindsight is a deterministic validator, compiler, and executor. The English → intent step is the model's job, on the caller's side.

## Roadmap

**v0**

- workspace, config, and data-dir resolution
- json / nginx / syslog / plaintext parsers with incremental ingest
- DuckDB store and the intent compiler (validation, deterministic compilation, read-only execution with caps)
- MCP server with `list_sources` / `describe` / `query(intent)` / `stats`
- `hindsight ingest` and `hindsight query`

**Later**

- live tailing and a follow mode
- k8s parser with label extraction
- `hindsight ask` built-in NL
- HTTP/SSE transport for the MCP server
- saved queries and named views
