# Monetization

Hindsight is open source (MIT) and self-hosted by default. This note records how it's meant to sustain itself without becoming the thing it argues against: a cloud that ingests everyone's logs and bills by the gigabyte.

## The stance

No multi-tenant log SaaS. Hindsight's whole pitch is that raw logs stay inside your perimeter. Hosting customer logs in our cloud would contradict that and drop us into the same cost structure as the incumbents. So we don't.

Instead: open core, with an optional managed control plane. The data plane always runs in the customer's environment.

## What's free

The core, for good:

- the Rust engine (ingest, follow, store, the query intent compiler)
- `hindsight-mcp` and `hindsight-cli`
- `hindsight-server` with token auth
- single-node, self-hosted use

Someone can run all of Hindsight on their own boxes and never pay us. That's intended. It's how the tool gets adopted.

## What's paid

A managed control plane and the team features around it:

- SSO and RBAC: who can query which sources
- a fleet view across many hosts and environments
- the audit trail of what every agent queried (the echo-backs), retained and exportable
- alert routing and saved queries
- retention and lifecycle policy management
- support and SLAs

The control plane is hosted by us. The logs are not.

## What stays local

The boundary is the whole point. The mechanics are in the "Trust and auth boundary" section of [ARCHITECTURE.md](ARCHITECTURE.md). In short:

- config and policy flow down from the control plane to the customer's servers
- health, metadata, and the query results a user explicitly asks for flow up
- raw logs never leave the customer's environment

So the paid product is management and compliance, not storage.

## Pricing

Per host (or per source, or per seat), not per gigabyte ingested. Two reasons:

- it's predictable, so the buyer knows the bill in advance
- "we don't charge by log volume" is a direct wedge against Datadog, Splunk, and every incumbent that does

## The compliance angle

The echo-back audit trail is a product on its own. In regulated shops, "prove which agent read which logs, and exactly what it asked" is something people pay for. It comes from the same data we already produce for trust, so turning it into a feature costs little.

## Non-goals

- hosting customer log data
- billing on ingest volume
- a closed core
