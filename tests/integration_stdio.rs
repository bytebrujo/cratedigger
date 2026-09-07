#![forbid(unsafe_code)]
pub mod support;

use serde_json::{Value, json};
use std::{process::Stdio, time::Duration};
use support::{Mock, Reply, TEST_USER_AGENT};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::timeout,
};

struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: Lines<BufReader<ChildStdout>>,
    next_id: u64,
}

impl Session {
    async fn start(base: Option<&str>) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mcp-crates"));
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(base) = base {
            command
                .env("CRATES_IO_BASE_URL", base)
                .env("MCP_CRATES_USER_AGENT", TEST_USER_AGENT);
        } else {
            command.env_remove("CRATES_IO_BASE_URL");
        }
        let mut child = command.spawn().unwrap();
        let stdin = Some(child.stdin.take().unwrap());
        let stdout = BufReader::new(child.stdout.take().unwrap()).lines();
        let mut session = Self {
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        let initialize: Value =
            serde_json::from_str(include_str!("fixtures/initialize.json")).unwrap();
        let response = session
            .request("initialize", initialize["params"].clone())
            .await;
        assert_eq!(response["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(response["result"]["serverInfo"]["name"], "mcp-crates");
        assert!(response["result"]["capabilities"]["tools"].is_object());
        session
            .send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
            .await;
        session
    }

    async fn send(&mut self, message: Value) {
        self.stdin
            .as_mut()
            .unwrap()
            .write_all(format!("{message}\n").as_bytes())
            .await
            .unwrap();
    }

    async fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .await;
        let line = timeout(Duration::from_secs(50), self.stdout.next_line())
            .await
            .expect("MCP call must have a bounded response")
            .unwrap()
            .expect("server exited early");
        let response: Value = serde_json::from_str(&line).expect("stdout is protocol JSON only");
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], id);
        response
    }

    async fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
            .await
    }

    async fn close(mut self) {
        drop(self.stdin.take());
        assert!(
            timeout(Duration::from_secs(5), self.child.wait())
                .await
                .unwrap()
                .unwrap()
                .success()
        );
        assert!(self.stdout.next_line().await.unwrap().is_none());
        let mut diagnostics = String::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut diagnostics)
            .await
            .unwrap();
        assert!(!diagnostics.is_empty());
        for line in diagnostics.lines() {
            let entry: Value = serde_json::from_str(line).expect("single-line JSON diagnostic");
            assert!(entry["ts"].is_u64());
            assert!(entry["level"].is_string());
            assert!(entry["msg"].is_string());
        }
    }
}

fn success(response: &Value) -> &Value {
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response["result"]["isError"], false, "{response}");
    let result = &response["result"]["structuredContent"];
    assert!(result.get("fetched_at").is_some(), "{response}");
    let text: Value =
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(&text, result);
    result
}

#[tokio::test]
async fn four_p0_tools_over_real_stdio() {
    let live = std::env::var("LIVE").as_deref() == Ok("1");
    let mock = (!live).then(Mock::fixtures);
    let mut session = Session::start(mock.as_ref().map(|m| m.url.as_str())).await;
    let listed = session.request("tools/list", json!({})).await;
    let tools = listed["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 4);
    for name in [
        "search_crates",
        "crate_info",
        "crate_versions",
        "resolve_version",
    ] {
        let tool = tools.iter().find(|tool| tool["name"] == name).unwrap();
        assert_eq!(tool["inputSchema"]["type"], "object");
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert!(tool["inputSchema"]["properties"].is_object());
        assert!(tool["inputSchema"]["required"].is_array());
        let output_schema = &tool["outputSchema"];
        assert_eq!(output_schema["type"], "object");
        let alternatives = output_schema["anyOf"].as_array().unwrap();
        assert_eq!(
            alternatives.len(),
            2,
            "success and structured failure schemas"
        );
        for alternative in alternatives {
            let shape = match alternative["$ref"].as_str() {
                Some(reference) => output_schema
                    .pointer(reference.trim_start_matches('#'))
                    .unwrap(),
                None => alternative,
            };
            assert_eq!(shape["type"], "object");
            assert!(shape["properties"].get("fetched_at").is_some());
        }
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
    }
    let search_schema = tools.iter().find(|t| t["name"] == "search_crates").unwrap();
    assert_eq!(
        search_schema["inputSchema"]["properties"]["limit"]["maximum"],
        20
    );
    let search = session
        .call("search_crates", json!({"query": "itoa", "limit": 3}))
        .await;
    let rows = success(&search)["crates"].as_array().unwrap();
    assert!(!rows.is_empty() && rows.len() <= 3);
    assert!(rows.iter().any(|row| row["name"] == "itoa"));
    let info = session.call("crate_info", json!({"name": "itoa"})).await;
    assert_eq!(success(&info)["name"], "itoa");
    assert!(success(&info)["features"].is_object());
    let versions = session
        .call("crate_versions", json!({"name": "itoa"}))
        .await;
    let version_rows = success(&versions)["versions"].as_array().unwrap();
    assert!(!version_rows.is_empty() && version_rows.len() <= 50);
    let resolved = session
        .call("resolve_version", json!({"name": "itoa", "req": "*"}))
        .await;
    assert_eq!(success(&resolved)["req_valid"], true);
    assert_eq!(
        success(&resolved)["resolved"],
        success(&info)["latest_version"]
    );
    assert_eq!(
        success(&resolved)["fetched_at"],
        success(&versions)["fetched_at"]
    );
    let repeat = session
        .call("search_crates", json!({"query": "itoa", "limit": 3}))
        .await;
    assert_eq!(success(&repeat), success(&search));
    let unknown = session.call("missing", json!({})).await;
    assert_eq!(unknown["error"]["code"], -32601);
    assert_eq!(unknown["error"]["data"]["field"], "name");
    if let Some(mock) = mock {
        let requests = mock.requests();
        assert_eq!(
            requests.len(),
            3,
            "metadata, search and versions only; resolver shares version cache"
        );
        for request in &requests {
            assert!(
                request
                    .headers
                    .to_ascii_lowercase()
                    .contains(&format!("user-agent: {TEST_USER_AGENT}"))
            );
        }
        for pair in requests.windows(2) {
            assert!(pair[1].at - pair[0].at >= Duration::from_millis(950));
        }
    }
    session.close().await;
}

#[tokio::test]
async fn invalid_fields_are_precise_and_do_not_hit_upstream() {
    let mock = Mock::fixtures();
    let mut session = Session::start(Some(&mock.url)).await;
    for (name, arguments, field) in [
        ("search_crates", json!({}), "query"),
        ("search_crates", json!({"query": 3}), "query"),
        ("search_crates", json!({"query": "   "}), "query"),
        ("search_crates", json!({"query": "é".repeat(257)}), "query"),
        (
            "search_crates",
            json!({"query": "itoa", "limit": 21}),
            "limit",
        ),
        (
            "search_crates",
            json!({"query": "itoa", "limit": 0}),
            "limit",
        ),
        (
            "search_crates",
            json!({"query": "itoa", "limit": 2.5}),
            "limit",
        ),
        (
            "search_crates",
            json!({"query": "itoa", "limit": null}),
            "limit",
        ),
        ("crate_info", json!({"name": "../itoa"}), "name"),
        ("crate_versions", json!({"name": "itoa/versions"}), "name"),
        ("crate_versions", json!({"name": 123}), "name"),
        ("resolve_version", json!({"name": "itoa", "req": 1}), "req"),
        ("crate_info", json!({"name": "itoa", "typo": true}), "typo"),
    ] {
        let response = session.call(name, arguments).await;
        assert_eq!(response["result"]["isError"], true, "{response}");
        assert_eq!(
            response["result"]["structuredContent"]["error"]["field"], field,
            "{response}"
        );
        assert_eq!(
            response["result"]["structuredContent"]["error"]["code"],
            "invalid_input"
        );
    }
    for req in ["", "not semver", "1 || 2"] {
        let response = session
            .call("resolve_version", json!({"name": "itoa", "req": req}))
            .await;
        let value = success(&response);
        assert_eq!(value["req_valid"], false);
        assert!(value["resolved"].is_null());
        assert!(value["fetched_at"].is_null());
        assert!(value["note"].as_str().unwrap().starts_with("req:"));
    }
    assert!(mock.requests().is_empty());
    session.close().await;
}

#[tokio::test]
async fn garbage_upstream_returns_structured_errors_and_server_recovers() {
    let mock = Mock::start(|request, index| {
        if index < 4 {
            Reply::json(if index % 2 == 0 {
                "<html>garbage</html>"
            } else {
                r#"{"unexpected":true}"#
            })
        } else {
            assert!(request.path.starts_with("/api/v1/crates?"));
            Reply::json(include_str!("fixtures/search_itoa.json"))
        }
    });
    let mut session = Session::start(Some(&mock.url)).await;
    for (name, arguments) in [
        ("search_crates", json!({"query": "itoa"})),
        ("crate_info", json!({"name": "itoa"})),
        ("crate_versions", json!({"name": "itoa"})),
        ("resolve_version", json!({"name": "itoa", "req": "*"})),
    ] {
        let response = session.call(name, arguments).await;
        assert_eq!(response["result"]["isError"], true, "{response}");
        assert_eq!(
            response["result"]["structuredContent"]["error"]["code"],
            "invalid_response"
        );
        assert!(response["result"]["structuredContent"]["fetched_at"].is_null());
    }
    let response = session
        .call("search_crates", json!({"query": "itoa"}))
        .await;
    assert!(!success(&response)["crates"].as_array().unwrap().is_empty());
    assert_eq!(
        mock.requests().len(),
        5,
        "invalid responses must not enter the cache"
    );
    session.close().await;
}
