# Announcement drafts

Prepared for use after crates.io publication is verified. Neither draft has
been posted. Confirm the destination community/account before posting.

## Rust community

**mcp-crates: crates.io lookup tools for MCP clients, written in Rust**

mcp-crates v0.1.0 exposes four read-only tools over stdio: search crates,
inspect metadata and feature flags, list versions, and resolve Cargo semver
requirements against non-yanked releases.

The server shares a 1 request/second limiter and TTL cache across tools, sends
an identifying User-Agent, bounds requests and responses, and returns structured
errors. It includes offline fixture and subprocess tests, Linux/macOS CI, and
live verification through the official MCP Inspector.

Install with `cargo install mcp-crates --locked` (Rust 1.98.1+).

Source and client configuration: https://github.com/bytebrujo/cratedigger

Crate: https://crates.io/crates/mcp-crates

The project also tracks an AI-assisted maintenance experiment through
2027-03-31. Feedback on the Rust lookup workflow and client compatibility is
welcome. Licensed MIT OR Apache-2.0.

## MCP community

**A Rust ecosystem lookup server for your MCP client**

mcp-crates v0.1.0 provides four stdio tools for agents working with Rust:
search_crates, crate_info, crate_versions and resolve_version.

Results include structured JSON, portable input/output schemas, and fetch
timestamps so clients can distinguish cached snapshots. Version resolution
uses Cargo's semver rules, including prerelease opt-in and yanked-version
filtering. Calls share a rate limiter and cache and have bounded timeouts.

The README includes Claude Desktop and Claude Code configurations. Install
with `cargo install mcp-crates --locked` using Rust 1.98.1 or later.

Source, setup and feedback: https://github.com/bytebrujo/cratedigger

Crate: https://crates.io/crates/mcp-crates

Tested through the official MCP Inspector. MIT OR Apache-2.0 licensed.
