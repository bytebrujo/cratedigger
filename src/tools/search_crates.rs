use super::Arguments;
use crate::{
    crates_io::{CratesIo, Fetched, SearchResponse},
    error::Error,
};
use rmcp::model::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

fn default_limit() -> u8 {
    10
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Input {
    #[schemars(length(min = 1, max = 256))]
    pub query: String,
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 20))]
    pub limit: u8,
}

#[derive(Serialize, JsonSchema)]
pub struct CrateSummary {
    pub name: String,
    pub description: Option<String>,
    pub latest_version: String,
    pub downloads: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct Output {
    #[schemars(length(max = 20))]
    pub crates: Vec<CrateSummary>,
    /// Unix seconds when crates.io returned this snapshot; retained on cache hits.
    pub fetched_at: u64,
}

pub fn definition() -> Tool {
    super::definition::<Input, Output>(
        "search_crates",
        "Search crates.io. Limit defaults to 10 (1–20). latest_version uses max_stable_version, falling back to max_version. fetched_at is Unix seconds; search cache TTL is 10 minutes.",
    )
}

pub async fn run(arguments: &Arguments, client: &CratesIo) -> Result<Value, Error> {
    super::fields(arguments, &["query", "limit"])?;
    let query = super::string(arguments, "query", 256)?;
    let limit = match arguments.get("limit") {
        None => default_limit(),
        Some(value) => value
            .as_u64()
            .filter(|n| (1..=20).contains(n))
            .ok_or_else(|| Error::input("limit", "must be an integer from 1 to 20"))?
            as u8,
    };
    super::json(shape(client.search(query.trim(), limit).await?, limit)?)
}

pub fn shape(response: Fetched<SearchResponse>, limit: u8) -> Result<Output, Error> {
    let crates = response
        .data
        .crates
        .into_iter()
        .take(usize::from(limit.min(20)))
        .map(|krate| {
            let latest_version = krate
                .max_stable_version
                .filter(|s| !s.is_empty())
                .unwrap_or(krate.max_version);
            semver::Version::parse(&latest_version)
                .map_err(|_| Error::InvalidResponse("invalid latest version in search".into()))?;
            Ok(CrateSummary {
                name: krate.name,
                description: krate.description,
                latest_version,
                downloads: krate.downloads,
                repository: krate.repository,
            })
        })
        .collect::<Result<_, Error>>()?;
    Ok(Output {
        crates,
        fetched_at: response.fetched_at,
    })
}
