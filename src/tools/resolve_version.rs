use super::Arguments;
use crate::{
    crates_io::{CratesIo, Fetched, VersionsResponse, validate_name},
    error::Error,
};
use rmcp::model::Tool;
use schemars::JsonSchema;
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Input {
    #[schemars(length(min = 1, max = 64), regex(pattern = "^[A-Za-z][A-Za-z0-9_-]*$"))]
    pub name: String,
    /// Cargo semver requirement; invalid syntax returns req_valid=false without a request.
    #[schemars(length(max = 256))]
    pub req: String,
}

#[derive(Serialize, JsonSchema)]
pub struct Output {
    pub resolved: Option<String>,
    pub req_valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Unix seconds of the version snapshot; null if invalid req avoided a request.
    pub fetched_at: Option<u64>,
}

pub fn definition() -> Tool {
    super::definition::<Input, Output>(
        "resolve_version",
        "Resolve a Cargo semver requirement to the highest non-yanked match across all versions, including versions beyond the 50-result display cap. Prereleases require explicit opt-in. Invalid syntax returns req_valid=false without a request, with fetched_at=null; otherwise fetched_at is Unix seconds.",
    )
}

pub async fn run(arguments: &Arguments, client: &CratesIo) -> Result<Value, Error> {
    super::fields(arguments, &["name", "req"])?;
    let name = super::string(arguments, "name", 64)?;
    validate_name(&name)?;
    let req = arguments
        .get("req")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::input("req", "required string"))?;
    if req.chars().count() > 256 || req.chars().any(char::is_control) {
        return Err(Error::input(
            "req",
            "must be at most 256 characters with no control characters",
        ));
    }
    let requirement = match VersionReq::parse(req) {
        Ok(requirement) => requirement,
        Err(error) => {
            return super::json(Output {
                resolved: None,
                req_valid: false,
                note: Some(format!("req: {error}")),
                fetched_at: None,
            });
        }
    };
    super::json(shape(client.versions(&name).await?, &requirement)?)
}

pub fn shape(
    response: Fetched<VersionsResponse>,
    requirement: &VersionReq,
) -> Result<Output, Error> {
    let resolved = super::sorted_versions(&response.data.versions)?
        .into_iter()
        .find(|(version, record)| !record.yanked && requirement.matches(version))
        .map(|(_, record)| record.num.clone());
    let note = resolved
        .is_none()
        .then(|| "No non-yanked version matches req.".into());
    Ok(Output {
        resolved,
        req_valid: true,
        note,
        fetched_at: Some(response.fetched_at),
    })
}
