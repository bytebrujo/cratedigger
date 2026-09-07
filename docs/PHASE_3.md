# Phase 3 review — 2026-09-06

Local hardening, documentation and real-client verification are complete.
The user confirmed the GitHub owner as bytebrujo. The repository now exists
as private at https://github.com/bytebrujo/cratedigger. Live verification passed;
branch protection is blocked by the GitHub plan, and hosted CI is being checked.

## Client compatibility finding and fix

The official MCP Inspector 2.5.0 connected to the release binary and discovered
all four tools. Its strict schema check initially reported 15 warnings: nullable
fields used array-valued type declarations, which some MCP clients mishandle.

Output schemas now express those type unions as anyOf branches. The conversion
uses Schemars' schema-aware traversal and preserves existing constraints,
required fields, null vs absent, and the wire response shape. A regression test
checks the generated tool schemas. Inspector then reported zero findings.

## Evidence

- make check passed: formatting, clippy with warnings denied, 28 offline tests,
  all cargo-deny checks, and a locked release build.
- Inspector ran all four tools against recorded crates.io fixtures through the
  actual stdio release executable. It also checked invalid semver syntax and an
  invalid search limit; both retained the expected client-visible behavior.
- Python jsonschema 4.26.0 independently validated every input/output schema
  and each response against its advertised output schema, including nullable
  results and structured errors. JSON text content matched structuredContent.
- A separate local installation was built with cargo install --path . --locked
  --offline --force into target/phase-3/install. The same Inspector checks were
  run against that installed executable.
- Inspector captures and executable SHA-256 hashes are saved in
  target/phase-3/inspector/ and target/phase-3/installed-inspector/.
  These generated files are ignored by Git; scripts/inspect_mcp.py reproduces
  them and shuts down its child processes and loopback fixture server.

The original checks used a local fixture upstream. After repository setup,
the LIVE=1 subprocess smoke also passed against crates.io, and Inspector ran
all four tools plus both negative cases against the live API with zero schema
findings. Captures are in target/phase-3/live-inspector/. This is not a Claude
Desktop UI test; hosted CI results are tracked on the implementation PR.

## GitHub setup and remaining gate

- Repository: https://github.com/bytebrujo/cratedigger (private by default;
  public visibility remains a separate decision).
- Default User-Agent uses Cargo repository/authors metadata and the existing
  Git contact, santiagoreyesanti@gmail.com. The environment override remains.
- The Actions User-Agent variable is configured, and Actions are allowed to
  create maintenance PRs. Workflow permissions default to read.
- A source-free root commit establishes main as the initial PR's base. All
  project files are introduced through the implementation PR.
- The main branch-protection request requires strict Linux/macOS check contexts,
  PRs, administrator enforcement, resolved conversations, and no force pushes
  or deletion. GitHub rejected it with HTTP 403: private-repository protection
  requires GitHub Pro or public visibility on the current plan.

Do not merge or publish until branch protection can be enabled. The remaining
choice is to make the repository public or keep it private with a GitHub plan
that supports protection. CI results should be read from the implementation PR.
Phase 4 tags, crates.io publication and announcements remain pending review.
PREDICTIONS.md, the seed maintenance template and original spec remain unchanged.
