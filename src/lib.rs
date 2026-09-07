#![forbid(unsafe_code)]

pub mod crates_io;
pub mod error;
pub mod tools;

use crates_io::CratesIo;
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
};

#[derive(Clone)]
pub struct CratesServer {
    client: CratesIo,
}

impl CratesServer {
    pub fn new(client: CratesIo) -> Self {
        Self { client }
    }
}

impl ServerHandler for CratesServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")))
            .with_instructions("Live crates.io lookup tools. fetched_at is Unix seconds of the cached upstream snapshot, or null when no snapshot was fetched. All requests share a 1 req/s limiter.")
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: tools::definitions(),
            ..Default::default()
        })
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        tools::definitions()
            .into_iter()
            .find(|tool| tool.name == name)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let arguments = request.arguments.unwrap_or_default();
        let result = match request.name.as_ref() {
            "search_crates" => tools::search_crates::run(&arguments, &self.client).await,
            "crate_info" => tools::crate_info::run(&arguments, &self.client).await,
            "crate_versions" => tools::crate_versions::run(&arguments, &self.client).await,
            "resolve_version" => tools::resolve_version::run(&arguments, &self.client).await,
            _ => return Err(error::Error::UnknownTool(request.name.into_owned()).into()),
        };
        Ok(match result {
            Ok(value) => CallToolResult::structured(value),
            Err(error) => error.tool_result(),
        }
        .into())
    }
}
