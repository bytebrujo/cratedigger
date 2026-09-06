# AGENTS.md — read this first, every session

## What this is
An MCP server (stdio) exposing crates.io/docs.rs lookup tools. The tool schemas
are the contract. stdout is protocol-only; all logs are single-line JSON on stderr.

## Commands
- make check   # fmt, clippy -D warnings, offline tests, cargo-deny, locked build
- make run     # serve on stdio for a local client

## Hard rules
1. Never merge with make check red. Never allow(warnings) to silence clippy —
   fix or justify with a Learned Rules entry.
2. unsafe_code is forbidden at crate root; do not relax it.
3. Respect crates.io etiquette: User-Agent + 1 req/s limiter + cache. Any new
   endpoint goes through src/crates_io.rs, nowhere else.
4. Live-network tests never gate PRs.
5. New/changed tool = update its schema, fixture tests, integration assertions,
   README table, and CHANGELOG in the same PR.
6. SDK upgrades: read the SDK changelog first, pin exactly, note breakage and
   fix pattern as a Learned Rules entry.

## Learned rules (append-only, date each entry, newest last)
<!-- appended by the maintaining agent -->

- 2026-09-06: Pin cargo-deny 0.20.2 in Makefile. Version 0.18.5 cannot
  parse CVSS 4.0 in the current RustSec database (RUSTSEC-2026-0073), failing
  before it can audit this project's dependencies. Update the auditing tool;
  never disable advisories or modify the upstream database to pass the gate.
- 2026-09-06: Allow only syn 2.0.119 alongside syn 3 in the duplicate-version
  ban. tracing-attributes 0.1.31 and Windows SDK support require syn 2, while
  current serde, schemars, tokio, and thiserror derives require syn 3. These
  are transitive build dependencies. Revisit the exact exception on updates.
- 2026-09-06: Add semver as the sole additional runtime dependency beyond the
  original allowlist. Its VersionReq implements Cargo's caret, tilde, wildcard,
  comparator and prerelease rules; do not maintain a handwritten substitute.
- 2026-09-06: reqwest 0.13 names its rustls feature `rustls` (formerly
  `rustls-tls`). Disable defaults, automatic retries, and redirects so every
  upstream attempt goes through the single application limiter.
- 2026-09-06: Permit MCP_CRATES_USER_AGENT as an environment-only setting
  until the project repository/contact is finalized. Public API calls require
  an identifying mcp-crates User-Agent with HTTPS URL and email. Never embed
  invented ownership/contact details. Loopback tests may use a test identity.
- 2026-09-06: Allow ISC, BSD-3-Clause and CDLA-Permissive-2.0 in cargo-deny
  for rustls's crypto, certificate validation and root-certificate data.
  These are transitive reqwest/rustls requirements. OpenSSL/native-tls remain
  banned, and no advisory exceptions were added.
- 2026-09-06: make check forces LIVE=0. Live smoke is an explicit separate
  invocation and weekly job; an inherited LIVE must not affect PR gates.
- 2026-09-06: Check version-list totals and pagination before caching. If
  upstream changes its unpaginated contract, fail visibly instead of resolving
  against an incomplete list or returning a false null.
- 2026-09-06: The std::net test stub must set accepted streams to blocking
  mode explicitly. macOS can inherit the listener's O_NONBLOCK flag; treating
  an early WouldBlock as EOF caused intermittent IncompleteMessage failures.
- 2026-09-06: Output schemas must cover both success and structured failures.
  Use the untagged ToolPayload schema so isError responses also conform to
  the advertised contract. Keep fetched_at=null when no snapshot was fetched.
- 2026-09-06: Inspector 2.5.0 flagged nullable type arrays as portability
  warnings. Convert schema type arrays to equivalent anyOf branches with
  Schemars' schema-aware traversal; preserve null vs absent and existing
  constraints. Response shapes must remain unchanged.
- 2026-09-06: Optional manual client verification uses pinned MCP Inspector
  2.5.0 and Python jsonschema 4.26.0 in scripts/inspect_mcp.py. These are
  inspection tools, not Rust/runtime dependencies or make check prerequisites.
  Fixture mode is the default; --live requires an explicit public identity.
- 2026-09-06: The user confirmed GitHub owner bytebrujo. The repository is
  bytebrujo/cratedigger, private pending a separate visibility/release decision.
  The default User-Agent uses Cargo's repository/authors metadata and the
  existing Git contact; an environment override remains available.
- 2026-09-06: A source-free root commit is used only to establish the main
  branch as the first PR's base. All project files are introduced through the
  initial implementation PR; the empty bootstrap is not a buildable release.
