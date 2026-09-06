#![forbid(unsafe_code)]

use mcp_crates::error::Error;
use rmcp::{ErrorData, model::ErrorCode};

#[test]
fn unknown_tool_names_the_invalid_field() {
    let error: ErrorData = Error::UnknownTool("missing".into()).into();
    assert_eq!(error.code, ErrorCode::METHOD_NOT_FOUND);
    assert_eq!(error.message, "name: unknown tool 'missing'");
    assert_eq!(error.data.unwrap()["field"], "name");
}
