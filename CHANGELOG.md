# Changelog

All notable changes are documented here using
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and semantic versioning.

## [Unreleased]

## [0.1.0] - 2026-09-07

### Added

- Four stdio MCP tools: search_crates, crate_info, crate_versions and
  resolve_version, with field validation and portable input/output schemas.
- Cargo-compatible semver resolution over complete non-yanked version lists.
- Shared 1 request/second rate limiting, TTL caching, bounded responses/cache,
  timeouts and up to three attempts for HTTP 429/5xx failures.
- Structured success/error results, original fetch timestamps and JSON-only
  stderr diagnostics.
- Repository/contact User-Agent by default and an environment override.
- Recorded fixtures, offline protocol tests and optional live MCP Inspector
  verification with independent JSON Schema validation.
- Linux/macOS CI, guarded crates.io publishing, weekly live smoke and dependency
  update PRs, with pinned Rust 1.98.1, rmcp 3.2.0 and cargo-deny 0.20.2.
- MIT OR Apache-2.0 licensing, client setup instructions and maintenance
  experiment records, with evaluation scheduled for 2027-03-31.
