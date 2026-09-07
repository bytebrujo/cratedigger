# Build Spec — crates.io / docs.rs Lookup MCP Server (Rust)

**Status:** ready to build · **Deliverable:** a published crate on crates.io providing an MCP (Model Context Protocol) server over stdio · **This document is self-contained.** Do not assume any other project or shared infrastructure exists.

---

## 1. Purpose and experiment

This repo is an experiment in **AI-first software design**: every decision optimizes for an AI agent building and maintaining the code. Governing principles:

1. **Training-data density** — boring, massively popular, stable-API technology only.
2. **Feedback-loop quality** — Rust is chosen precisely because the compiler and clippy convert whole error classes into fast, loud, structured text; the toolchain is the agent's pair programmer.

The product: an MCP server that gives any MCP client (Claude, editors, agents) live lookup of the Rust crates ecosystem — search, metadata, versions, feature flags, docs links, dependency freshness. Maintenance drift is guaranteed and external: the MCP spec is still revving, the official Rust SDK takes breaking changes, and crates.io API policy evolves.

The experiment runs 6 months from first publish. Evaluation date: **2027-03-31**.

## 2. Ground rules (non-negotiable)

- **Toolchain:** stable Rust, pinned via `rust-toolchain.toml` (current stable at build time). Edition 2021 or later per SDK requirement.
- **Dependencies (allowlist):** the official MCP Rust SDK (`rmcp` — verify current name/version on crates.io at build time and pin exactly), `tokio`, `reqwest` (rustls-tls, no openssl), `serde`/`serde_json`, `thiserror`, `schemars` if the SDK uses it for tool schemas. Dev-deps: none beyond the SDK's test needs. Anything else requires an `AGENTS.md` Learned Rules entry.
- **`#![forbid(unsafe_code)]`** at crate root.
- **Gates:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo deny check` (licenses + advisories + bans), `cargo build --locked`. All inside `make check`; all are unmergeable CI gates.
- **Transport:** stdio only in v1. No HTTP transport, no auth.
- **All diagnostics to stderr as single-line JSON** (`{"ts":…,"level":…,"msg":…}`); stdout belongs exclusively to the MCP protocol.
- **All changes land via PR with green CI**; branch protection from day 1.
- **License:** MIT OR Apache-2.0 (standard Rust dual license).

## 3. External API etiquette (hard requirements)

- **crates.io:** send a descriptive `User-Agent` including the repo URL and a contact address (their policy requires it); global rate limit of **1 request/second** enforced in code with a token-bucket; in-memory TTL cache (search 10 min, crate metadata 1 h, version lists 1 h) so bursty agent usage doesn't hit the network.
- **docs.rs:** construct canonical URLs; any page fetches get the same User-Agent and rate limiting.
- On upstream 429/5xx: exponential backoff (max 3 tries), then a structured tool error the client can show — never a panic, never a hang.

## 4. Tool surface (exact)

All tools return structured JSON content. Input schemas are declared through the SDK so clients can validate.

**P0 (v0.1.0 ships with exactly these):**

| tool | input | output |
|---|---|---|
| `search_crates` | `{query: string, limit?: int ≤20}` | `{crates:[{name, description, latest_version, downloads, repository?}]}` |
| `crate_info` | `{name: string}` | `{name, description, latest_version, license, repository?, homepage?, documentation?, features: {feature: [deps]}, yanked_versions: [string], keywords, categories}` |
| `crate_versions` | `{name: string}` | `{versions:[{num, yanked, msrv?, published_at}]}` (newest first, cap 50) |
| `resolve_version` | `{name: string, req: string}` | `{resolved: string \| null, req_valid: bool, note?}` — semver resolution against non-yanked versions |

**P1 (only after month 1 maintenance runs clean):**

- `check_manifest` — input `{cargo_toml: string}`; parses `[dependencies]` tables, returns per-dep `{current_req, latest, latest_matching_req, yanked: bool, behind: major|minor|patch|none}`.
- `docs_url` — input `{name, version?, item_path?}`; returns the canonical docs.rs URL (constructed, existence-checked with a HEAD request).

**P2 (design headroom only, do not build):** RUSTSEC advisory lookup; reverse-dependency counts.

Every tool: validate input first and return a precise error naming the field; cap all list outputs; include a `fetched_at` timestamp so clients can reason about staleness.

## 5. Architecture

```
mcp-crates/                       # final crate name decided at publish, §12
├── AGENTS.md / PREDICTIONS.md / MAINTENANCE_LOG.md    # verbatim in §9
├── README.md                     # includes client-config snippets (§8)
├── Makefile                      # §7
├── Cargo.toml / Cargo.lock       # lockfile committed (it's a binary)
├── rust-toolchain.toml
├── deny.toml
├── src/
│   ├── main.rs                   # stdio server bootstrap, tool registration
│   ├── tools/                    # one module per tool; thin: parse → call client → shape
│   ├── crates_io.rs              # typed client: rate limiter + cache + endpoints
│   └── error.rs                  # thiserror types; one conversion path to MCP errors
├── tests/
│   ├── fixtures/*.json           # recorded crates.io responses
│   ├── unit_*.rs                 # parsing/shaping against fixtures — no network
│   └── integration_stdio.rs      # §6
└── .github/workflows/            # ci.yml, release.yml, weekly.yml — §8
```

Locality rule: an agent editing a tool should touch **one file** in `src/tools/` plus its test. No trait gymnastics, no macro magic beyond what the SDK requires; if the SDK offers a derive-macro path and an explicit path, prefer the explicit one.

## 6. Testing (the contract in executable form)

1. **Unit tests, offline:** every crates.io response shape is a committed fixture; parsing and output-shaping tested against them. A `scripts/record_fixture.sh <endpoint>` helper (curl with the proper User-Agent) refreshes fixtures deliberately.
2. **Integration test, offline:** `integration_stdio.rs` spawns the compiled binary as a child process and speaks real JSON-RPC over its stdin/stdout: `initialize` → `tools/list` (assert all P0 tools with schemas) → `tools/call` for each tool with a mock upstream (inject base URL via env var pointing at a local axum-free hyper stub or a static-file server — pick the simplest thing that needs no new deps; a tiny `std::net::TcpListener` responder is acceptable and preferred).
3. **Live smoke, scheduled not gating:** `weekly.yml` runs the same integration against the real crates.io behind `LIVE=1`, opening a `maintenance` issue on failure. Live tests never gate PRs (flaky network must not train anyone to override red CI).

## 7. `make check`

```
make check:   cargo fmt --check
              cargo clippy --all-targets -- -D warnings
              cargo test                     # offline only
              cargo deny check
              cargo build --locked --release
```

Green from fresh clone with only the pinned toolchain installed. `make run` starts the server on stdio for manual client testing.

## 8. CI, release, and client config

- `ci.yml` — every PR/push: `make check` on Linux; add a macOS job (cheap portability signal).
- `release.yml` — on tag `v*`: `make check`, then `cargo publish` using `CARGO_REGISTRY_TOKEN` secret. Tag only after CHANGELOG.md entry exists (keep-a-changelog format; CI greps for the version).
- `weekly.yml` — live smoke (§6.3) + `cargo update` PR labeled `maintenance`.
- **README must include copy-pasteable client config** for Claude Desktop / Claude Code (`command: "mcp-crates"` via `cargo install`), because install friction is the difference between having users (real maintenance pressure) and not.

## 9. Verbatim files

### 9.1 `AGENTS.md` seed

```markdown
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
```

### 9.2 `PREDICTIONS.md`

```markdown
# PREDICTIONS.md — written before first line of code. DO NOT EDIT. Evaluate 2027-03-31.

Window: first publish → 2027-03-31. “Unattended” = green CI, zero human edits
to code (merge clicks and prompt-sending excluded).

P1  ≥80% of external-drift fixes (MCP SDK/spec upgrades, crates.io API changes)
    complete unattended. Measure: MAINTENANCE_LOG entries tagged drift-fix.
P2  Human interventions per monthly run decline: month 6 < month 2.
P3  Zero releases published with red or overridden CI; zero clippy suppressions
    without a Learned Rules entry. Measure: git/CI history audit at evaluation.
P4  Median human minutes per maintenance run ≤15 from month 3 onward.
P5  ≥1 external user signal (issue/PR/usage report from a stranger) by evaluation.

Verdict per prediction at evaluation: HELD / FALSIFIED / INCONCLUSIVE + evidence.
```

### 9.3 `MAINTENANCE_LOG.md` entry template

```markdown
## 2026-MM-DD — run N
- prompt: standing (unchanged) | special: <text>
- tasks attempted / unattended / interventions (with one-line why):
- human_minutes: N · tokens/cost if known:
- drift encountered: <sdk|spec|crates.io|none> — details
- AGENTS.md rules added: N · release cut: yes/no (version)
- CI green on main at end: yes/no
```

### 9.4 Standing monthly prompt (verbatim, never reworded)

```
Run the monthly maintenance pass for this repo:
1. cargo update; upgrade the MCP SDK if a new version exists (read its changelog
   first); keep make check green.
2. Run the live smoke locally; fix any upstream drift.
3. Address open issues labeled maintenance.
4. If any user-visible change occurred, update CHANGELOG and cut a release tag.
5. Leave CI green on main.
6. Append a MAINTENANCE_LOG.md entry using the template. Do not edit PREDICTIONS.md.
```

## 10. Build phases and acceptance criteria

**Phase 1 — skeleton + protocol proof (day 1).**
- [ ] Repo, branch protection, full tree from §5, verbatim files from §9.
- [ ] Server boots on stdio; `initialize` + `tools/list` answered correctly (empty-ish toolset OK); integration test harness runs it as a child process.
- [ ] `make check` green.

**Phase 2 — P0 tools (day 2–3).**
- [ ] Four P0 tools implemented through `src/crates_io.rs` with limiter + cache.
- [ ] Fixture unit tests + full stdio integration coverage per §6.
- [ ] Kill test: point base-URL env at a responder returning garbage; confirm structured tool errors, no panic, no hang.

**Phase 3 — hardening + docs (day 3–4).**
- [ ] README: what/why, tool table, client config snippets, install via `cargo install`.
- [ ] `cargo deny` configured; CHANGELOG started; macOS CI job green.
- [ ] Manual end-to-end: connect from a real MCP client, run every tool.

**Phase 4 — publish (day 5).**
- [ ] Name finalized (§12), `v0.1.0` tagged, `release.yml` publishes to crates.io.
- [ ] Announced in at least two places Rust/MCP users gather.
- [ ] MAINTENANCE_LOG run 0 (build) recorded.

## 11. Non-goals (v1)

- No HTTP/SSE transport, no auth, no multi-tenant anything.
- No disk cache, no config file (env vars only: base URLs for tests, `LIVE`).
- No RUSTSEC/advisory tooling (P2).
- No Windows CI in v1 (revisit if a user asks — that's the experiment working).

## 12. Decisions deferred to the build session

- Verify the official Rust MCP SDK's current crate name, version, and API shape on crates.io/GitHub before writing any code; pin exactly.
- Crate name: check availability of `mcp-crates`, `crates-lookup-mcp`, `cargo-mcp-lookup`; the binary name should match the crate.
- Confirm current crates.io API paths and rate-limit policy from their published docs; record in `src/crates_io.rs` doc comments.

## 13. Session kickoff prompt (paste to start the build)

```
Read AGENTS.md if present, then the build spec at <path-to-this-file>. First
resolve every item in §12 and show me your findings. Then execute Phase 1;
stop for my review at the end of each phase. make check must be green at every
commit.
```