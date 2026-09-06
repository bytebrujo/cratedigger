#![forbid(unsafe_code)]

use std::{process::ExitCode, time::SystemTime};

use mcp_crates::{CratesServer, crates_io::CratesIo};
use rmcp::{ServiceExt, transport::stdio};
use serde_json::json;

fn diagnostic(level: &str, message: &str) {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    eprintln!("{}", json!({"ts": ts, "level": level, "msg": message}));
}

#[tokio::main]
async fn main() -> ExitCode {
    std::panic::set_hook(Box::new(|info| diagnostic("error", &info.to_string())));
    diagnostic("info", "starting mcp-crates on stdio");
    let client = match CratesIo::from_env() {
        Ok(client) => client,
        Err(error) => {
            diagnostic("error", &error.to_string());
            return ExitCode::FAILURE;
        }
    };
    let service = match CratesServer::new(client).serve(stdio()).await {
        Ok(service) => service,
        Err(error) => {
            diagnostic("error", &format!("MCP initialization failed: {error}"));
            return ExitCode::FAILURE;
        }
    };
    match service.waiting().await {
        Ok(_) => {
            diagnostic("info", "MCP connection closed");
            ExitCode::SUCCESS
        }
        Err(error) => {
            diagnostic("error", &format!("MCP service failed: {error}"));
            ExitCode::FAILURE
        }
    }
}
