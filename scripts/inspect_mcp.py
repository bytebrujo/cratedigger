#!/usr/bin/env python3
"""Optional real-client verification; never part of make check.

Requires Node >=22.19, npm, and Python jsonschema (validated with 4.26.0).
Runs pinned official MCP Inspector 2.5.0 against recorded fixtures by default.
Use --live for deliberate live verification (uses the binary's default identity
unless MCP_CRATES_USER_AGENT is supplied).
"""

import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import threading
import time

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[1]
INSPECTOR = "@modelcontextprotocol/inspector@2.5.0"
TEST_IDENTITY = "mcp-crates/0.1.0 (https://example.invalid/test-repository; tests@example.invalid)"


class FixtureHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path.startswith("/api/v1/crates?"):
            filename = "search_itoa.json"
        else:
            filename = {
                "/api/v1/crates/itoa": "crate_itoa.json",
                "/api/v1/crates/itoa/versions": "versions_itoa.json",
            }.get(self.path)
        if filename is None:
            self.send_error(404)
            return
        body = (ROOT / "tests/fixtures" / filename).read_bytes()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


def run_inspector(config, output, label, method, arguments=None):
    command = ["npx", "--yes", INSPECTOR, "--cli", "--config", str(config),
               "--server", "crates", "--method", method, "--format", "json"]
    if method == "tools/list":
        command.append("--strict")
    else:
        command.extend(["--tool-name", label.split("--")[0], "--tool-args-json", json.dumps(arguments)])
    process = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, text=True, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=120)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate()
        raise RuntimeError(f"Inspector timed out: {label}") from None
    (output / f"{label}.json").write_text(stdout)
    (output / f"{label}.stderr").write_text(stderr)
    if process.returncode not in (0, 5):
        raise RuntimeError(f"Inspector {label} exited {process.returncode}: {stderr[-1500:]}")
    return process.returncode, json.loads(stdout)


def inspect(binary, output, live):
    output.mkdir(parents=True, exist_ok=True)
    server = None
    worker = None
    if live:
        identity = os.environ.get("MCP_CRATES_USER_AGENT")
        environment = {"CRATES_IO_BASE_URL": "https://crates.io"}
        if identity:
            environment["MCP_CRATES_USER_AGENT"] = identity
    else:
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), FixtureHandler)
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        environment = {"MCP_CRATES_USER_AGENT": TEST_IDENTITY,
                       "CRATES_IO_BASE_URL": f"http://127.0.0.1:{server.server_port}"}
    try:
        with tempfile.TemporaryDirectory(prefix="mcp-crates-inspect-") as temporary:
            config = Path(temporary) / "mcp.json"
            config.write_text(json.dumps({"mcpServers": {"crates": {
                "command": str(binary), "args": [], "env": environment}}}))
            config.chmod(0o600)
            _, listed = run_inspector(config, output, "tools-list", "tools/list")
            tools = {tool["name"]: tool for tool in listed["result"]["tools"]}
            assert set(tools) == {"search_crates", "crate_info", "crate_versions", "resolve_version"}
            findings = [finding for tool in listed.get("schemaFindings", []) for finding in tool["findings"]]
            assert not findings, findings
            for tool in tools.values():
                Draft202012Validator.check_schema(tool["inputSchema"])
                Draft202012Validator.check_schema(tool["outputSchema"])
            checks = []
            for label, arguments, expected_error in [
                ("search_crates", {"query": "itoa", "limit": 3}, False),
                ("crate_info", {"name": "itoa"}, False),
                ("crate_versions", {"name": "itoa"}, False),
                ("resolve_version", {"name": "itoa", "req": "*"}, False),
                ("resolve_version--invalid", {"name": "itoa", "req": "not semver"}, False),
                ("search_crates--invalid", {"query": "itoa", "limit": 21}, True),
            ]:
                # Each Inspector command starts a new server. Space live runs
                # across those separate processes, in addition to their limiter.
                if live:
                    time.sleep(1)
                code, envelope = run_inspector(config, output, label, "tools/call", arguments)
                name = label.split("--")[0]
                if not expected_error:
                    Draft202012Validator(tools[name]["inputSchema"]).validate(arguments)
                result = envelope["result"]
                assert bool(result.get("isError")) == expected_error, result
                assert code == (5 if expected_error else 0), code
                payload = result["structuredContent"]
                Draft202012Validator(tools[name]["outputSchema"]).validate(payload)
                assert json.loads(result["content"][0]["text"]) == payload
                if label == "resolve_version--invalid":
                    assert payload["req_valid"] is False and payload["fetched_at"] is None
                if expected_error:
                    assert payload["error"]["field"] == "limit"
                checks.append({"case": label, "passed": True, "is_error": expected_error})
                print(f"PASS {label}", flush=True)
            summary = {"inspector": "2.5.0", "live": live, "binary": str(binary),
                       "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                       "schema_findings": findings, "checks": checks}
            (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    finally:
        if server:
            server.shutdown()
            server.server_close()
            worker.join()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--live", action="store_true")
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/mcp-crates")
    parser.add_argument("--output", type=Path, default=ROOT / "target/phase-3/inspector")
    args = parser.parse_args()
    inspect(args.binary.resolve(strict=True), args.output.resolve(), args.live)
