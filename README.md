# Hindsight

Natural-language log analyzer. Logs are a record of what already happened, and Hindsight is how you interrogate that record.

Hindsight ingests logs in any format (JSON, nginx, syslog, k8s) into a local DuckDB store, then answers plain-English questions by generating the query for you. Ask "every 500 in the last hour grouped by endpoint" and it returns the result without you reconstructing the exact grep. Everything runs on your machine, so nothing gets shipped to a third party.

## Status

Early development. Not yet usable.

## Planned v0

- Ingest common log formats into a local DuckDB store
- Plain-English question → generated SQL → result
- Local-first: no data leaves the machine

## License

MIT
