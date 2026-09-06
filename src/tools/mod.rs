//! Each tool owns its input/output schemas, validation, and output shaping.
pub mod crate_info;
pub mod crate_versions;
pub mod resolve_version;
pub mod search_crates;

use crate::{crates_io::VersionRecord, error::Error};
use rmcp::model::{Tool, ToolAnnotations};
use schemars::{JsonSchema, Schema};
use serde::Serialize;
use serde_json::{Map, Value};

pub type Arguments = Map<String, Value>;

pub fn definitions() -> Vec<Tool> {
    vec![
        search_crates::definition(),
        crate_info::definition(),
        crate_versions::definition(),
        resolve_version::definition(),
    ]
}

fn definition<I: JsonSchema + 'static, O: JsonSchema + 'static>(
    name: &'static str,
    description: &'static str,
) -> Tool {
    let mut tool = Tool::new(name, description, Map::new())
        .with_input_schema::<I>()
        .with_output_schema::<crate::error::ToolPayload<O>>()
        .with_annotations(
            ToolAnnotations::new()
                .read_only(true)
                .destructive(false)
                .idempotent(true),
        );
    // Both alternatives are objects. Keep the root explicit for clients using
    // the older MCP outputSchema contract that requires type=object.
    if let Some(schema) = &mut tool.output_schema {
        std::sync::Arc::make_mut(schema).insert("type".into(), Value::String("object".into()));
        let mut portable = Schema::from(schema.as_ref().clone());
        portable_nullable_types(&mut portable);
        *schema = std::sync::Arc::new(portable.ensure_object().clone());
    }
    tool
}

/// Some MCP clients only accept a string-valued `type`. Schemars' schema-aware
/// traversal avoids rewriting data inside examples, defaults, or enum values.
fn portable_nullable_types(schema: &mut Schema) {
    schemars::transform::transform_subschemas(&mut portable_nullable_types, schema);
    if let Some(types) = schema.get("type").and_then(Value::as_array).cloned() {
        schema.remove("type");
        let alternatives: Vec<_> = types
            .into_iter()
            .map(|kind| serde_json::json!({"type": kind}))
            .collect();
        if schema.get("anyOf").is_some() {
            // Preserve existing alternatives as a separate AND constraint.
            let mut constraints = match schema.remove("allOf") {
                Some(Value::Array(constraints)) => constraints,
                _ => Vec::new(),
            };
            constraints.push(serde_json::json!({"anyOf": alternatives}));
            schema.insert("allOf".into(), Value::Array(constraints));
        } else {
            schema.insert("anyOf".into(), Value::Array(alternatives));
        }
    }
}

fn fields(arguments: &Arguments, allowed: &[&str]) -> Result<(), Error> {
    if let Some(field) = arguments
        .keys()
        .find(|field| !allowed.contains(&field.as_str()))
    {
        return Err(Error::input(field, "unknown field"));
    }
    Ok(())
}

fn string(arguments: &Arguments, field: &str, max: usize) -> Result<String, Error> {
    let value = arguments
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::input(field, "required string"))?;
    if value.trim().is_empty() || value.chars().count() > max || value.chars().any(char::is_control)
    {
        return Err(Error::input(
            field,
            &format!(
                "must contain 1–{max} characters, including non-whitespace, and no control characters"
            ),
        ));
    }
    Ok(value.into())
}

fn json<T: Serialize>(value: T) -> Result<Value, Error> {
    serde_json::to_value(crate::error::ToolPayload::Success(value))
        .map_err(|e| Error::InvalidResponse(e.to_string()))
}

/// Reject malformed upstream semver rather than silently changing resolution.
fn sorted_versions(
    versions: &[VersionRecord],
) -> Result<Vec<(semver::Version, &VersionRecord)>, Error> {
    let mut parsed: Vec<_> = versions
        .iter()
        .map(|record| {
            semver::Version::parse(&record.num)
                .map(|version| (version, record))
                .map_err(|_| Error::InvalidResponse(format!("invalid version num: {}", record.num)))
        })
        .collect::<Result<_, _>>()?;
    parsed.sort_by(|(a, _), (b, _)| b.cmp(a));
    Ok(parsed)
}
