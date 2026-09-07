# Build review — 2026-09-06

## Decisions resolved before implementation

- Official SDK: `rmcp = "=3.2.0"`, verified directly through the crates.io API
  and [official release](https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.2.0).
  It declares edition 2024 and MSRV 1.88. Explicit `ServerHandler` methods are
  available. Disable default features and enable only `server`, `transport-io`.
  This excludes HTTP/auth transports and avoids the SDK tool macros.
  Implement `get_tool` with the same definitions used by `list_tools` in Phase 2.
  Explicit handlers must also validate arguments themselves; schema discovery
  is not a replacement for runtime validation.
- Toolchain: stable Rust 1.98.1, released 2026-09-03, verified against the
  [Rust release notes](https://doc.rust-lang.org/releases.html) and rustup manifest.
- Package/binary: `mcp-crates`. All three proposed names returned HTTP 404
  from `https://crates.io/api/v1/crates/{name}`. Availability is not reservation;
  check again before publication. Workspace/repository may remain `cratedigger`.
- API routes: search `/api/v1/crates`, metadata `/api/v1/crates/{name}`, versions
  `/api/v1/crates/{name}/versions`, verified against the official
  [controllers](https://github.com/rust-lang/crates.io/tree/main/src/controllers/krate).
  Version sorting supports `date` and `semver`; explicit `per_page` enables pagination.
- [Data-access policy](https://crates.io/data-access) requires an identifying
  User-Agent and maximum 1 request/second; contact information is recommended.
  The project's requirement to include both repository and email is stricter.

## Review findings and implementation decisions

1. `resolve_version` needs the widely used `semver` crate to implement Cargo
   requirements correctly. Add it in Phase 2 with an append-only Learned Rule;
   do not handwrite a semver parser.
2. A pinned compiler alone does not supply `cargo-deny`. `make check` installs
   a pinned cargo-deny when missing or at a different version. Make and normal
   platform build prerequisites are still required. Offline *tests* do not mean
   first-time dependency installation or advisory updates work offline.
3. Output capping must happen after resolution. A valid old requirement must
   not return null just because it falls outside the newest 50 versions.
4. Phase 2 should define `latest_version` as the newest non-yanked stable
   release, falling back to a prerelease only if no stable release exists.
   Features and license must come from that same version. Define timestamp
   format, nested-list bounds, and truncation behavior in the tool schemas.
5. Rate limiting must cover retries globally, with a one-token capacity to
   prevent bursts. Bound upstream time, response size, and cache growth. A
   concurrent cache miss must not bypass the limiter.
6. Phase 1 has no upstream calls, so its fixtures cover the protocol only.
   Recorded crates.io responses, the fixture recorder, live smoke, and all four
   tools belong to Phase 2. Weekly live smoke stays explicitly disabled until
   it exercises real lookups. Never silently pass an empty live test.
7. GitHub owner/visibility and public contact are pending user confirmation.
   Local implementation and checks can proceed independently. No repository
   URL or contact is invented in the distributable package.
8. Follow the explicit phase checkpoints in §13. Phase 1 is reviewable before
   implementing P0. Publication, release tags, and community announcements
   remain later-phase work.

## Phase 1 acceptance record

- Original spec, verbatim seed files, and monthly prompt saved before Rust code.
- Explicit SDK handler, stdio executable, central routing error conversion.
- Child-process integration tests with bounded execution and cleanup.
- Linux/macOS CI, guarded release workflow, weekly dependency-update workflow.
- `make check` passed locally on macOS/aarch64 with Rust 1.98.1: formatting,
  clippy with warnings denied, two offline tests, all cargo-deny checks, and
  the locked release build. No warnings remained in the final gate run.
- Verbatim seed preservation and workflow YAML syntax were checked separately.
- Git is initialized on `phase-1/protocol-skeleton`; changes remain uncommitted
  for review. There is no remote yet. Repository creation, branch protection,
  PR creation, and hosted Linux/macOS CI remain pending repository details.
  Local success is not evidence of hosted CI success.

## Initial maintenance drift

The first gate run passed compilation, clippy, and offline tests, then found
that cargo-deny 0.18.5 cannot parse current RustSec CVSS 4.0 advisories. The
Makefile now pins 0.20.2, verified from crates.io and the
[cargo-deny changelog](https://github.com/EmbarkStudios/cargo-deny/blob/main/CHANGELOG.md).
The lockfile also needs syn 2 and 3 because upstream procedural macros are
mid-migration. A narrowly pinned duplicate-version exception is recorded in
AGENTS.md; advisory checks remain enabled.
