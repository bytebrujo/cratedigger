use super::Arguments;
use crate::{
    crates_io::{CratesIo, Fetched, VersionsResponse, validate_name},
    error::Error,
};
use rmcp::model::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Input {
    #[schemars(length(min = 1, max = 64), regex(pattern = "^[A-Za-z][A-Za-z0-9_-]*$"))]
    pub name: String,
}

#[derive(Serialize, JsonSchema)]
pub struct Version {
    pub num: String,
    pub yanked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msrv: Option<String>,
    pub published_at: String,
}

#[derive(Serialize, JsonSchema)]
pub struct Output {
    #[schemars(length(max = 50))]
    pub versions: Vec<Version>,
    /// Unix seconds when the response was fetched; retained on cache hits.
    pub fetched_at: u64,
}

pub fn definition() -> Tool {
    super::definition::<Input, Output>(
        "crate_versions",
        "List the highest 50 crate versions in descending semantic-version order, including yanked versions and optional MSRV. published_at is the upstream RFC3339 timestamp. fetched_at is Unix seconds; cache TTL 1 hour.",
    )
}

pub async fn run(arguments: &Arguments, client: &CratesIo) -> Result<Value, Error> {
    super::fields(arguments, &["name"])?;
    let name = super::string(arguments, "name", 64)?;
    validate_name(&name)?;
    super::json(shape(client.versions(&name).await?)?)
}

pub fn shape(response: Fetched<VersionsResponse>) -> Result<Output, Error> {
    let versions = super::sorted_versions(&response.data.versions)?
        .into_iter()
        .take(50)
        .map(|(_, record)| Version {
            num: record.num.clone(),
            yanked: record.yanked,
            msrv: record.rust_version.clone(),
            published_at: record.created_at.clone(),
        })
        .collect();
    Ok(Output {
        versions,
        fetched_at: response.fetched_at,
    })
}
