# Changelog

All notable changes will be documented here using
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and semantic versioning.

## [Unreleased]

### Added

- Phase 1 stdio MCP skeleton using exactly pinned rmcp 3.2.0 and Rust 1.98.1.
- Offline subprocess protocol tests and JSON stderr diagnostics.
- Linux/macOS quality gates, release validation, and weekly dependency updates.
- Experiment predictions, maintenance template, and original build specification.
- Four P0 tools with explicit SDK input/output schemas and field-specific errors.
- Shared crates.io limiter, TTL cache, response/cache bounds, timeouts, and
  up to three attempts for HTTP 429/5xx.
- Cargo-compatible semver resolution across the complete version list.
- Recorded fixtures, subprocess tool coverage, and local HTTP failure tests.
- Deliberate fixture recorder and enabled weekly live-smoke harness.
- Environment-supplied public User-Agent identity pending repository details.
- Optional pinned MCP Inspector runner for independent client/schema checks,
  using local fixtures by default and supporting explicit live verification.

### Changed

- Nullable output schemas use portable anyOf branches instead of type arrays,
  preserving the existing success/error response contract.
- The server supplies its repository/contact User-Agent by default; client
  configuration no longer requires an environment setting for live lookups.
