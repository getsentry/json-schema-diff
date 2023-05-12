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
