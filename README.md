# mcp-crates

A Rust MCP server over stdio for crates.io and docs.rs lookups. This project
tests whether an agent can maintain a small, strongly checked tool through
upstream SDK and API changes until 2027-03-31.

**Current status: v0.1.0 release preparation.** All four tools passed local,
Linux/macOS CI and live MCP-client verification. The repository is public at
[bytebrujo/cratedigger](https://github.com/bytebrujo/cratedigger), with protected
main. The crates.io upload is pending; see [release status](docs/RELEASE.md).

## Run locally

Install [rustup](https://rustup.rs/) and Make, then run:

```sh
make check
cargo install --path . --locked
mcp-crates
```

`rust-toolchain.toml` pins Rust 1.98.1, rustfmt, and clippy. `make check` installs
cargo-deny 0.20.2 if needed, checks formatting, lints, offline protocol tests,
licenses/advisories/dependency bans, and a locked release build. First setup
needs internet for toolchains, dependencies, and the RustSec advisory database;
the tests themselves never call crates.io. `make check` forces `LIVE=0`, even
when your shell has it enabled. `make run` starts the server locally. rustls's
crypto backend requires a normal platform C build toolchain.

The server sends an identifying User-Agent built from its package version,
repository URL and maintainer contact. Set `MCP_CRATES_USER_AGENT` only to
override that identity; overrides must include mcp-crates/version, an HTTPS
repository URL and contact email.

stdout contains MCP JSON-RPC only. Diagnostics are one JSON object per stderr
line with `ts` (Unix seconds), `level`, and `msg`.

## P0 tool contract

Each tool advertises input and output JSON schemas (including portable nullable
types and structured failures) and validates fields before
fetching. Names are 1–64 ASCII letters/digits/hyphens/underscores, starting with
a letter. Unknown fields, invalid types and out-of-range values are errors.

| Tool | Input | Result |
| --- | --- | --- |
| `search_crates` | Nonblank `query` (up to 256 characters), optional integer `limit` (1–20, default 10) | Up to `limit` crate descriptions, latest versions, downloads, repositories |
| `crate_info` | `name` | Metadata, latest-version features/license, yanked versions, keywords, categories |
| `crate_versions` | `name` | Up to 50 versions in descending semver order, with yanked status, optional MSRV, RFC3339 publication time |
| `resolve_version` | `name`, `req` | Highest non-yanked Cargo-semver match, requirement validity, optional note |

Successful results include `fetched_at` as Unix seconds of the upstream
snapshot, preserved on cache hits. It is null when no snapshot was fetched
(errors or invalid requirement syntax). Structured JSON is returned as both
MCP `structuredContent` and a JSON text block for client compatibility.

`crate_info` chooses the highest non-yanked stable version, with prerelease
fallback, and takes features/license from that same version. With no non-yanked
version it returns an error. Yanked versions are capped at the highest 50 by
semver. Feature maps allow 256 keys and 256 entries per feature; keywords and
categories allow 20 entries each. Exceeding these metadata bounds is an error.
Search uses upstream `max_stable_version`, falling back to `max_version`.

`resolve_version` checks the complete version list, including versions beyond
the display cap. It uses Cargo caret, tilde, wildcard and comparator semantics
with explicit prerelease opt-in. Invalid syntax (including empty requirements)
returns `req_valid: false` and an explanatory `note` without a request. A valid
requirement with no match returns `resolved: null, req_valid: true`.

Operational failures set MCP `isError: true` and return
`{error: {code, field, message, retryable}, fetched_at: null}`. Unknown tools
use a JSON-RPC routing error. The server stays alive after tool failures.

`check_manifest` and `docs_url`
remain deferred until month 1 maintenance runs clean; no advisory or reverse
dependency tools are planned for v0.1.0.

## Client configuration

The local installation above works now. After the
first release, install the published binary with:

```sh
cargo install mcp-crates --locked
```

For Claude Desktop, add this to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "crates": {
      "command": "mcp-crates",
      "args": []
    }
  }
}
```

If the desktop process does not inherit your shell PATH, use the absolute path
to the installed binary (normally `~/.cargo/bin/mcp-crates`, expanding `~`).

For Claude Code, add this to the project's `.mcp.json`:

```json
{
  "mcpServers": {
    "crates": {
      "type": "stdio",
      "command": "mcp-crates",
      "args": []
    }
  }
}
```

Configuration references: [Claude Code MCP](https://code.claude.com/docs/en/mcp)
and [MCP local server setup](https://modelcontextprotocol.io/docs/develop/connect-local-servers).

## Development and release

All tools share one 1 request/second capacity-one token bucket and cache.
Search TTL is 10 minutes; metadata and versions TTL is 1 hour. Concurrent
identical misses are coalesced. Cache limits: 128 entries and 32 MiB of bodies.

Requests use rustls, a 3-second connect timeout, 10-second request/body timeout,
8 MiB response limit, and 45-second lookup deadline including queue and retries.
HTTP 429/5xx get at most three attempts with 1- then 2-second backoff. Numeric
`Retry-After` values may extend the cooldown across calls; long waits return an
error. Other statuses and network failures return errors immediately. Redirects
and automatic HTTP-client retries are disabled. Incomplete version lists return
errors rather than incorrect semver results.

`CRATES_IO_BASE_URL` can override the origin for tests with a numeric loopback
HTTP(S) address. The normal origin is `https://crates.io`. `LIVE` affects only the
test harness, not the server. No config files or disk cache are used.

Run live smoke deliberately:

```sh
LIVE=1 cargo test --locked --test integration_stdio four_p0_tools_over_real_stdio -- --nocapture
```

Refresh recorded responses deliberately, following the
[fixture provenance](tests/fixtures/README.md):

```sh
scripts/record_fixture.sh '/api/v1/crates/itoa/versions' > tests/fixtures/versions_itoa.json
```

See [the original build spec](docs/BUILD_SPEC.md),
[build review](docs/BUILD_REVIEW.md), and [agent rules](AGENTS.md).
The experiment predictions were copied before implementation and must not be
edited. The exact monthly prompt is in [docs/MONTHLY_PROMPT.md](docs/MONTHLY_PROMPT.md).

All code changes must arrive through pull requests with green Linux and macOS
`make check` jobs. GitHub branch protection must require both checks and PRs,
include administrators, and disallow force pushes and deletion. These
protections are enabled on the public repository.

The release workflow checks that a tag matches Cargo.toml and a dated
Keep a Changelog entry, requires the tagged commit to be on main, runs all
gates on both platforms, then publishes with the `CARGO_REGISTRY_TOKEN`
repository secret. Publishing is restricted to crates.io. A release tag must
not be pushed until the secret is configured and the release PR has green CI.

The weekly workflow runs live smoke and opens a `maintenance` issue on failure.
Set its Actions variable `MCP_CRATES_USER_AGENT` to the agreed public identity.
It separately proposes dependency updates and explicitly dispatches both
platform CI checks on the dependency branch. Enable the repository's Actions
permission to create pull requests. See the Phase 3 review for hosted setup status.

See [Phase 2 review](docs/PHASE_2.md) for implementation decisions and validation.

For independent MCP-client and JSON Schema verification, use the optional
[MCP Inspector](https://github.com/modelcontextprotocol/inspector) runner.
It needs Node >=22.19/npm and Python jsonschema; these are not prerequisites
for the Rust build or `make check`:

```sh
python3 -m venv target/inspect-venv
target/inspect-venv/bin/pip install 'jsonschema==4.26.0'
target/inspect-venv/bin/python scripts/inspect_mcp.py
```

The runner downloads Inspector 2.5.0 through npx on first use, uses recorded
fixtures by default, checks all four tools plus error cases, and saves results
under `target/phase-3/inspector/`. Add `--live` for deliberate live verification. See [Phase 3 review](docs/PHASE_3.md) for evidence and pending setup.

Licensed under MIT OR Apache-2.0.
