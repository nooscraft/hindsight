# Hindsight architecture

Three decisions drive everything below.

1. **Rust core.** Ingestion, storage, and query live in a Rust workspace. It ships as a single binary with no runtime or interpreter.
2. **Local-first.** Logs are parsed and stored on the user's machine in an embedded DuckDB file. Nothing leaves the machine unless the user explicitly connects an external agent.
3. **Agent-first.** The primary interface is a tool surface an AI agent connects to, not a chat box or a dashboard. An agent can discover what logs exist, read their shape, and run queries through a few well-defined calls.

## Workspace layout

```
crates/
  hindsight-core   library: ingest + store + query
  hindsight-mcp    binary:  MCP server, the agent surface
  hindsight-cli    binary `hindsight`: human commands + ingestion
```

`hindsight-core` holds all the logic. The two binaries are thin: one speaks MCP to agents, the other speaks to a person at a terminal. Both call the same core.

## How local works

Data lives under a single directory, resolved in this order:

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

## Query and safety

All agent access to data is read-only. The query layer:

- accepts a single SQL statement, parses it, and rejects anything that isn't a `SELECT` (no DDL, DML, `PRAGMA`, `ATTACH`, `COPY`, or file functions)
- runs against DuckDB with a statement timeout
- caps result rows and total bytes, and reports truncation in the response

In agent-first mode the model writes the SQL. The core treats that SQL as untrusted and constrains it rather than trusting the caller.

## The agent surface

`hindsight-mcp` exposes tools over MCP, stdio to start and HTTP/SSE later. The goal is that an agent can assess a system's logs with minimal ceremony.

- `list_sources()` — each source with row count and time range
- `describe(source?)` — columns, detected `fields` keys with inferred types, and a few sample values per field
- `query(sql)` — run a read-only `SELECT`; returns rows plus a `truncated` flag
- `search(source?, text, since?, until?, level?)` — builds the `SELECT` for common lookups
- `stats(source, field)` — cardinality and top values for a field
- `sample(source, n)` — n recent rows

Normal agent loop: `list_sources` to see what's here, `describe` to learn the shape, then `query` or `search` to answer the question. Because the schema is discoverable, the agent writes correct SQL without anything being hard-coded.

## Where the natural language lives

Two ways to ask a question in plain English:

1. **Agent-first (primary).** An external agent (Grok, Claude, whatever the user runs) connects to `hindsight-mcp` and drives the tools. Hindsight ships no model and holds no API key.
2. **Built-in NL (optional).** `hindsight ask "..."` with a user-supplied API key runs the discover → generate SQL → execute loop in-process, for people who just want a CLI.

The core is identical either way. Built-in NL is an in-process agent using the same tools, under the same read-only guarantees.

## Roadmap

**v0**

- workspace, config, and data-dir resolution
- json / nginx / syslog / plaintext parsers with incremental ingest
- DuckDB store and read-only query layer with caps
- MCP server with `list_sources` / `describe` / `query` / `search`
- `hindsight ingest` and `hindsight query`

**Later**

- live tailing and a follow mode
- k8s parser with label extraction
- `hindsight ask` built-in NL
- HTTP/SSE transport for the MCP server
- saved queries and named views
