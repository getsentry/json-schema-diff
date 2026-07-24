use insta::{assert_debug_snapshot, glob, with_settings};
use json_schema_diff::diff;
use serde_json::Value;

#[test]
fn test_from_fixtures() {
    let test = |path: &std::path::Path| {
        let contents = std::fs::read_to_string(path).unwrap();
        let values: Value = serde_json::from_str(&contents).unwrap();
        let diff = diff(values["lhs"].clone(), values["rhs"].clone()).unwrap();
        with_settings!({ info => &values }, {
            assert_debug_snapshot!(diff);
        });
    };
    glob!("../tests/fixtures", "**/*.json", test);
}

#[test]
fn object_any_of_equals_property_with_multiple_types() {
    let lhs = serde_json::json!({
        "anyOf": [
            {"properties": {"foo": {"type": "integer"}}},
            {"properties": {"foo": {"type": "string"}}}
        ]
    });
    let rhs = serde_json::json!({
        "properties": {
            "foo": {
                "type": ["integer", "string"]
            }
        }
    });

    assert_eq!(diff(lhs.clone(), rhs.clone()).unwrap(), Vec::new());
    assert_eq!(diff(rhs, lhs).unwrap(), Vec::new());
}

#[test]
fn correlated_object_any_of_types_are_not_collapsed() {
    let lhs = serde_json::json!({
        "anyOf": [
            {
                "properties": {
                    "foo": {"type": "integer"},
                    "bar": {"type": "string"}
                }
            },
            {
                "properties": {
                    "foo": {"type": "string"},
                    "bar": {"type": "integer"}
                }
            }
        ]
    });
    let rhs = serde_json::json!({
        "properties": {
            "foo": {"type": ["integer", "string"]},
            "bar": {"type": ["integer", "string"]}
        }
    });

    assert!(!diff(lhs, rhs).unwrap().is_empty());
}
