# Fixture provenance

Raw JSON responses recorded on 2026-09-06 using scripts/record_fixture.sh,
one request at a time. Recording identified the build-session maintainer
via their GitHub profile and contact email. The distributable project identity
remains pending. No authentication or private data was used. Unknown upstream
fields are preserved.

| File | Endpoint | SHA-256 |
| --- | --- | --- |
| search_itoa.json | /api/v1/crates?q=itoa&per_page=3 | bee0def1237fd24414171a1f8f28624152e3b0d3b7515b72fd337afa605ac166 |
| crate_itoa.json | /api/v1/crates/itoa | b908ee352eb9b8a5ebcf6bcdc7b2f0584893440c34e98356cc7edb35e002872e |
| versions_itoa.json | /api/v1/crates/itoa/versions | e3fc02304b5e4f5dfe61cc9a2355bbd45fa527be7a253b407f11e2946071082c |

initialize.json is an authored MCP request fixture, not an upstream recording.

Unit tests derive synthetic variants for yanks, prereleases, old matches,
missing MSRV and limits. Failure tests explicitly synthesize HTTP errors,
oversized bodies and garbage. Refreshing fixtures requires reviewing changed
expected versions/timestamps and updating these hashes.
