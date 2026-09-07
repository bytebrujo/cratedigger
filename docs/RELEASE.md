# v0.1.0 release preparation

Repository: https://github.com/bytebrujo/cratedigger

Package and binary: mcp-crates. The name returned HTTP 404 from crates.io on
2026-09-07 and remains available at this checkpoint; availability is not a
reservation. Recheck before the first upload.

## Completed before the release PR

- Implementation PR #1 was merged with passing Linux/macOS checks.
- The repository is public and main branch protection is enabled, including
  strict required checks, PRs, administrator enforcement and no force pushes.
- All four P0 tools passed live subprocess and official MCP Inspector checks.
- The manifest restricts publishing to crates.io; the dated v0.1.0 changelog
  describes the final tool contract and release contents.
- The release workflow requires its tag to be on main and runs make check on
  Linux and macOS before cargo publish. The publish step fails clearly when
  the required secret is missing.
- CI caches the pinned cargo-deny executable, without caching credentials or
  skipping quality gates. The first uncached hosted run took 6–9 minutes.

## Publication blocker

No CARGO_REGISTRY_TOKEN was present in the local environment, Cargo credentials
file, or repository Actions secrets when release preparation started.

Create a crates.io account using the intended GitHub owner, verify its email,
and create an API token with permission to publish the new mcp-crates crate.
Add the token directly as the CARGO_REGISTRY_TOKEN repository Actions secret:

https://github.com/bytebrujo/cratedigger/settings/secrets/actions

Never paste the token into chat, source files, PR descriptions or logs. GitHub
secret listings can verify its presence without exposing its value.

## Final release sequence

1. Require a passing local make check and cargo publish --dry-run --locked.
2. Merge the release PR only after its protected Linux/macOS checks pass.
3. Confirm the secret exists, main CI is green, and the crate name is available.
4. Tag the checked main commit v0.1.0 and push that tag; release.yml publishes.
5. Verify the release workflow succeeds, crates.io exposes version 0.1.0 with
   the expected repository, and cargo install mcp-crates --version 0.1.0 --locked
   succeeds in a clean installation directory.
6. Test the registry-installed binary through MCP Inspector, record the actual
   publish outcome in the maintenance log, and publish the release notes.
7. Complete the two community announcements after publication. Draft text is
   prepared in ANNOUNCEMENTS.md; it must not be presented as already posted.

No release tag should be pushed while the credential or required checks are
missing. A dry run, merged PR, or announcement draft is not a published crate.

Reference: [Cargo publishing guide](https://doc.rust-lang.org/cargo/reference/publishing.html).
