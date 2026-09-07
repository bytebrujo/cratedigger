#![forbid(unsafe_code)]

use mcp_crates::tools;
use serde_json::Value;

#[test]
fn output_schemas_preserve_nullability_without_array_valued_types() {
    fn visit(value: &Value) {
        match value {
            Value::Object(object) => {
                assert!(!object.get("type").is_some_and(Value::is_array), "{value}");
                for child in object.values() {
                    visit(child);
                }
            }
            Value::Array(array) => {
                for child in array {
                    visit(child);
                }
            }
            _ => {}
        }
    }
    for tool in tools::definitions() {
        let schema = Value::Object(tool.output_schema.unwrap().as_ref().clone());
        visit(&schema);
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["anyOf"].as_array().unwrap().len(), 2);
        if tool.name == "resolve_version" {
            let resolved = &schema["$defs"]["Output"]["properties"]["resolved"];
            assert!(
                resolved["anyOf"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|branch| branch["type"] == "null")
            );
            assert!(
                resolved["anyOf"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|branch| branch["type"] == "string")
            );
        }
    }
}
