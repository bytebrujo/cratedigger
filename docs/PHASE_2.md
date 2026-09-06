# Phase 2 review — 2026-09-06

The four P0 tools are implemented locally. No P1/P2 tools were added, no release
was cut, and no public repository/contact information was invented.

## What changed

- Each file in `src/tools/` owns one tool's schema, validation and shaping.
  Shared helpers handle field extraction, serialization and semantic sorting.
  The SDK uses explicit handlers; exactly four tools are advertised.
- `src/crates_io.rs` owns typed endpoints, a shared capacity-one token bucket,
  cache, retries, deadlines, and origin/identity configuration.
- Search cache TTL is 600 seconds; metadata/versions TTL is 3600 seconds.
  Original fetched_at timestamps survive cache hits. Cache storage is capped at
  128 entries and 32 MiB of response bodies. Failed responses are not cached.
- Requests have a 3-second connect timeout, 10-second request/body timeout,
  8 MiB body limit, and 45-second overall lookup deadline including queueing.
  HTTP 429/5xx get at most three attempts with exponential backoff; numeric
  Retry-After cooldowns persist across calls. Redirects and automatic reqwest
  retries are disabled. No OpenSSL/native-tls dependencies are permitted.
- Semver resolution considers all upstream versions, not the 50-row display
  subset. Totals and pagination are checked before caching; partial responses
  fail visibly instead of changing the answer.
- Operational errors return structured MCP tool errors. Output schemas cover
  both success and failure while retaining an object root for older clients.
- Recorded crates.io fixtures and a deliberate refresh script are present.
  The weekly live smoke is enabled and is separate from offline quality gates.
  `make check` explicitly sets LIVE=0.

## Decisions made explicit

- Newest version means descending semantic version, not most recently uploaded
  historical maintenance release. crate_info selects a non-yanked stable
  version, then falls back to a prerelease; its features/license use that version.
  Search follows the upstream max_stable_version/max_version fields.
- Yanked-version output is capped at 50. Features allow 256 keys and 256 entries
  per key; keywords/categories allow 20 entries each. Excess metadata produces
  an error, avoiding silently incomplete feature flags.
- fetched_at is Unix seconds. It is null for errors and invalid requirements
  that avoid fetching. Upstream publication timestamps remain RFC3339 strings.
- The client serializes cache misses through one async mutex, coalescing
  concurrent misses without introducing per-key task management. A cache hit
  can wait behind an in-flight lookup; the overall deadline bounds that wait.
- semver is the only added runtime dependency outside the original allowlist.
  reqwest 0.13's rustls feature spelling and TLS license requirements are
  documented in AGENTS.md.
- MCP_CRATES_USER_AGENT is an explicit environment-only addition while the
  public repository/contact is unresolved. Missing identity produces a tool
  configuration error before network access. Numeric loopback test origins
  may use a test identity; arbitrary remote origins are rejected.

## Validation

The complete make check sequence passed locally on macOS/aarch64: formatting,
clippy with warnings denied, 27 offline tests, cargo-deny licenses/advisories/
bans/sources, and a locked release build.

Coverage includes:

- Real JSON-RPC child-process initialization, schemas, all four calls,
  text/structured-content compatibility, field errors, clean shutdown and logs.
- Malformed JSON and wrong response shapes for every tool, followed by a
  successful call proving the process remains alive and failures are not cached.
- Caret, tilde, wildcard/comparator, prerelease, yanked, no-match and older-than-
  display-cap semver cases; recorded response parsing and metadata selection.
- Cache timestamps, TTL/size/count limits, concurrent miss coalescing, URL
  encoding, one-second spacing, retries, cooldowns, request and queue deadlines.
- HTTP 404/redirect behavior, oversized responses and incomplete pagination.

An intermittent stub failure was traced to macOS accepted sockets inheriting
O_NONBLOCK. Accepted streams now explicitly use blocking reads/writes, and a
fragmented-header regression test covers the issue. After the fix, all three
stdio tests passed three consecutive runs before the complete gate run.

Raw upstream fixtures were fetched during this phase. The real-server LIVE=1
smoke and a real MCP host/client review have not been performed. Those results
must not be inferred from offline protocol coverage. Workflow YAML and client
JSON syntax were checked; hosted Linux/macOS CI has not run.

## Handoff

Phase 3 can review real-client behavior and harden the final public setup.
Repository owner/name/visibility and public contact are still unresolved.
After they are supplied, configure the User-Agent and Actions variable, create
the remote/PR, enforce branch protection, and run hosted CI and live smoke.

Changes remain local and uncommitted. PREDICTIONS.md and the original spec
remain unchanged. The Phase 4 release/publish/announcement steps remain pending.
