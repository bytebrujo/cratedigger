use super::Arguments;
use crate::{
    crates_io::{CratesIo, Fetched, MetadataResponse, validate_name},
    error::Error,
};
use rmcp::model::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Input {
    #[schemars(length(min = 1, max = 64), regex(pattern = "^[A-Za-z][A-Za-z0-9_-]*$"))]
    pub name: String,
}

#[derive(Serialize, JsonSchema)]
pub struct Output {
    pub name: String,
    pub description: Option<String>,
    pub latest_version: String,
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    /// At most 256 feature keys and 256 dependencies per feature; overflow is an error.
    pub features: BTreeMap<String, Vec<String>>,
    #[schemars(length(max = 50))]
    pub yanked_versions: Vec<String>,
    #[schemars(length(max = 20))]
    pub keywords: Vec<String>,
    #[schemars(length(max = 20))]
    pub categories: Vec<String>,
    /// Unix seconds; metadata cache TTL is one hour.
    pub fetched_at: u64,
}

pub fn definition() -> Tool {
    super::definition::<Input, Output>(
        "crate_info",
        "Crate metadata and features/license for the highest non-yanked stable version (prerelease fallback). Yanked versions: highest 50 by semver. Keywords/categories: up to 20 each. Feature maps over 256 keys or entries per feature return an error. fetched_at is Unix seconds; cache TTL 1 hour.",
    )
}

pub async fn run(arguments: &Arguments, client: &CratesIo) -> Result<Value, Error> {
    super::fields(arguments, &["name"])?;
    let name = super::string(arguments, "name", 64)?;
    validate_name(&name)?;
    super::json(shape(client.metadata(&name).await?)?)
}

pub fn shape(response: Fetched<MetadataResponse>) -> Result<Output, Error> {
    let parsed = super::sorted_versions(&response.data.versions)?;
    let latest = parsed
        .iter()
        .find(|(version, record)| !record.yanked && version.pre.is_empty())
        .or_else(|| parsed.iter().find(|(_, record)| !record.yanked))
        .map(|(_, record)| *record)
        .ok_or_else(|| Error::InvalidResponse("crate has no non-yanked version".into()))?;
    if latest.features.len() > 256
        || latest.features.values().any(|deps| deps.len() > 256)
        || response.data.keywords.len() > 20
        || response.data.categories.len() > 20
    {
        return Err(Error::InvalidResponse(
            "metadata exceeds feature (256) or keyword/category (20) output bounds".into(),
        ));
    }
    Ok(Output {
        name: response.data.krate.name,
        description: response.data.krate.description,
        latest_version: latest.num.clone(),
        license: latest.license.clone(),
        repository: response.data.krate.repository,
        homepage: response.data.krate.homepage,
        documentation: response.data.krate.documentation,
        features: latest.features.clone(),
        yanked_versions: parsed
            .iter()
            .filter(|(_, record)| record.yanked)
            .take(50)
            .map(|(_, record)| record.num.clone())
            .collect(),
        keywords: response
            .data
            .keywords
            .into_iter()
            .map(|keyword| keyword.keyword)
            .collect(),
        categories: response
            .data
            .categories
            .into_iter()
            .map(|category| category.slug)
            .collect(),
        fetched_at: response.fetched_at,
    })
}
