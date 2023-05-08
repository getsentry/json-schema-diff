#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

use schemars::schema::RootSchema;
use serde_json::Value;
use thiserror::Error;

mod diff_walker;
mod types;

pub use types::*;

/// Take two JSON schemas, and compare them.
///
/// `lhs` (left-hand side) is the old schema, `rhs` (right-hand side) is the new schema.
pub fn diff(lhs: Value, rhs: Value) -> Result<Vec<Change>, Error> {
    let changes = Vec::new();
    let lhs_root: RootSchema = serde_json::from_value(lhs)?;
    let rhs_root: RootSchema = serde_json::from_value(rhs)?;

    let mut walker = diff_walker::DiffWalker {
        changes,
        lhs_root,
        rhs_root,
    };
    walker.diff(
        "",
        &mut walker.lhs_root.schema.clone(),
        &mut walker.rhs_root.schema.clone(),
    )?;

    Ok(walker.changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    use insta::assert_debug_snapshot;

    #[test]
    fn nothing() {
        let lhs = json! {{ "type": "string" }};
        let rhs = json! {{ "type": "string" }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @"[]"
        );
    }

    #[test]
    fn basic() {
        let lhs = json! {{ "type": "string" }};
        let rhs = json! {{ "type": "number" }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: TypeRemove {
                    removed: String,
                },
            },
            Change {
                path: "",
                change: TypeAdd {
                    added: Number,
                },
            },
            Change {
                path: "",
                change: TypeAdd {
                    added: Integer,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn extend_type() {
        let lhs = json! {{ "type": "string" }};
        let rhs = json! {{ "type": ["string", "number"] }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: TypeAdd {
                    added: Number,
                },
            },
            Change {
                path: "",
                change: TypeAdd {
                    added: Integer,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn extend_from_const() {
        let lhs = json! {{ "const": "hello" }};
        let rhs = json! {{ "type": ["string"] }};

        let diff = diff(lhs, rhs).unwrap();

        // TODO: better support for const
        assert_debug_snapshot!(
            diff,
            @"[]"
        );
    }

    #[test]
    fn restrict_to_const() {
        let lhs = json! {{ "type": ["string"] }};
        let rhs = json! {{ "const": "hello" }};

        let diff = diff(lhs, rhs).unwrap();

        // TODO: better support for const
        assert_debug_snapshot!(
            diff,
            @"[]"
        );
    }

    #[test]
    fn additional_properties_extend() {
        let lhs = json! {{ "additionalProperties": false }};
        let rhs = json! {{ "additionalProperties": true }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: ".<additionalProperties>",
                change: TypeAdd {
                    added: String,
                },
            },
            Change {
                path: ".<additionalProperties>",
                change: TypeAdd {
                    added: Number,
                },
            },
            Change {
                path: ".<additionalProperties>",
                change: TypeAdd {
                    added: Integer,
                },
            },
            Change {
                path: ".<additionalProperties>",
                change: TypeAdd {
                    added: Object,
                },
            },
            Change {
                path: ".<additionalProperties>",
                change: TypeAdd {
                    added: Array,
                },
            },
            Change {
                path: ".<additionalProperties>",
                change: TypeAdd {
                    added: Boolean,
                },
            },
            Change {
                path: ".<additionalProperties>",
                change: TypeAdd {
                    added: Null,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn additional_properties_restrict() {
        let lhs = json! {{ "additionalProperties": true }};
        let rhs = json! {{ "additionalProperties": false }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: ".<additionalProperties>",
                change: TypeRemove {
                    removed: String,
                },
            },
            Change {
                path: ".<additionalProperties>",
                change: TypeRemove {
                    removed: Number,
                },
            },
            Change {
                path: ".<additionalProperties>",
                change: TypeRemove {
                    removed: Integer,
                },
            },
            Change {
                path: ".<additionalProperties>",
                change: TypeRemove {
                    removed: Object,
                },
            },
            Change {
                path: ".<additionalProperties>",
                change: TypeRemove {
                    removed: Array,
                },
            },
            Change {
                path: ".<additionalProperties>",
                change: TypeRemove {
                    removed: Boolean,
                },
            },
            Change {
                path: ".<additionalProperties>",
                change: TypeRemove {
                    removed: Null,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn integer_to_number() {
        let lhs = json! {{ "type": "integer" }};
        let rhs = json! {{ "type": "number" }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: TypeAdd {
                    added: Number,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn number_to_integer() {
        let lhs = json! {{ "type": "number" }};
        let rhs = json! {{ "type": "integer" }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: TypeRemove {
                    removed: Number,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn add_property() {
        let lhs = json! {{
            "type": "object",
            "properties": {
                "hello": {"type": "string"},
            }
        }};
        let rhs = json! {{
            "type": "object",
            "properties": {
                "hello": {"type": "string"},
                "world": {"type": "string"},
            }
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: PropertyAdd {
                    lhs_additional_properties: true,
                    added: "world",
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn remove_property() {
        let lhs = json! {{
            "type": "object",
            "properties": {
                "hello": {"type": "string"},
                "world": {"type": "string"},
            }
        }};
        let rhs = json! {{
            "type": "object",
            "properties": {
                "hello": {"type": "string"},
            }
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: PropertyRemove {
                    lhs_additional_properties: true,
                    removed: "world",
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn change_property() {
        let lhs = json! {{
            "type": "object",
            "properties": {
                "hello": {"type": "string"},
            }
        }};
        let rhs = json! {{
            "type": "object",
            "properties": {
                "hello": {"type": "number"},
            }
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: ".hello",
                change: TypeRemove {
                    removed: String,
                },
            },
            Change {
                path: ".hello",
                change: TypeAdd {
                    added: Number,
                },
            },
            Change {
                path: ".hello",
                change: TypeAdd {
                    added: Integer,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn add_property_in_array_of_anyof() {
        // rough shape of a sentry eventstream message
        // https://github.com/getsentry/sentry-kafka-schemas/pull/79/files
        let lhs = json! {{
            "anyOf": [
                {
                    "type": "array",
                    "items": [
                        {"const": "start_unmerge"},
                        {"type": "object"}
                    ]
                }
            ]
        }};

        let rhs = json! {{
            "anyOf": [
                {
                    "type": "array",
                    "items": [
                        {"const": "start_unmerge"},
                        {"type": "object", "properties": {"transaction_id": {"type": "string"}}}
                    ]
                }
            ]
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(diff, @r###"
        [
            Change {
                path: ".<anyOf:0>.1",
                change: PropertyAdd {
                    lhs_additional_properties: true,
                    added: "transaction_id",
                },
            },
        ]
        "###);
    }

    #[test]
    fn remove_property_while_allowing_additional_properties() {
        let lhs = json! {{
            "type": "object",
            "properties": {
                "foobar": {"type": "string"}
            },
            "additionalProperties": true
        }};

        let rhs = json! {{
            "type": "object",
            "additionalProperties": true
        }};

        let diff = diff(lhs, rhs).unwrap();
        assert_debug_snapshot!(diff, @r###"
        [
            Change {
                path: "",
                change: PropertyRemove {
                    lhs_additional_properties: true,
                    removed: "foobar",
                },
            },
        ]
        "###);
    }

    #[test]
    fn add_property_in_array() {
        let lhs = json! {{
            "type": "array",
            "items": [
                {"const": "start_unmerge"},
                {"$ref": "#/definitions/Hello"}
            ],
            "definitions": {
                "Hello": {
                    "type": "object",
                }
            }
        }};

        let rhs = json! {{
            "type": "array",
            "items": [
                {"const": "start_unmerge"},
                {"$ref": "#/definitions/Hello"}
            ],
            "definitions": {
                "Hello": {
                    "type": "object",
                    "properties": {"transaction_id": {"type": "string"}}
                }
            }
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(diff, @r###"
        [
            Change {
                path: ".1",
                change: PropertyAdd {
                    lhs_additional_properties: true,
                    added: "transaction_id",
                },
            },
        ]
        "###);
    }

    #[test]
    fn factor_out_definitions() {
        let lhs = json! {{
            "type": "object"
        }};

        let rhs = json! {{
            "$ref": "#/definitions/Hello",
            "definitions": {
                "Hello": {"type": "object"}
            }
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(diff, @"[]");
    }

    #[test]
    fn factor_out_definitions_and_change() {
        let lhs = json! {{
            "type": "object"
        }};

        let rhs = json! {{
            "$ref": "#/definitions/Hello",
            "definitions": {
                "Hello": {"type": "array"}
            }
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(diff, @r###"
        [
            Change {
                path: "",
                change: TypeRemove {
                    removed: Object,
                },
            },
            Change {
                path: "",
                change: TypeAdd {
                    added: Array,
                },
            },
        ]
        "###);
    }

    #[test]
    fn any_of_order_change() {
        let lhs = json! {{
            "anyOf": [
                {"type": "array"},
                {"type": "string"},
            ]
        }};

        let rhs = json! {{
            "anyOf": [
                {"type": "string"},
                {"type": "array"},
            ]
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(diff, @"[]");
    }

    #[test]
    fn add_minimum() {
        let lhs = json! {{
            "type": "number",
        }};
        let rhs = json! {{
            "type": "number",
            "minimum": 1.0
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: RangeAdd {
                    added: Minimum,
                    value: 1.0,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn add_minimum_in_array() {
        let lhs = json! {{
            "type": "array",
            "items": [
                {"const": "start_unmerge"},
                {"$ref": "#/definitions/Hello"}
            ],
            "definitions": {
                "Hello": {
                    "type": "number",
                }
            }
        }};

        let rhs = json! {{
            "type": "array",
            "items": [
                {"const": "start_unmerge"},
                {"$ref": "#/definitions/Hello"}
            ],
            "definitions": {
                "Hello": {
                    "type": "number",
                    "minimum": 1.0
                }
            }
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(diff, @r###"
        [
            Change {
                path: ".1",
                change: RangeAdd {
                    added: Minimum,
                    value: 1.0,
                },
            },
        ]
        "###);
    }

    #[test]
    fn add_maximum() {
        let lhs = json! {{
            "type": "number",
        }};
        let rhs = json! {{
            "type": "number",
            "maximum": 1.0
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: RangeAdd {
                    added: Maximum,
                    value: 1.0,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn remove_minimum() {
        let lhs = json! {{
            "type": "number",
            "minimum": 1.0
        }};
        let rhs = json! {{
            "type": "number",
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: RangeRemove {
                    removed: Minimum,
                    value: 1.0,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn remove_maximum() {
        let lhs = json! {{
            "type": "number",
            "maximum": 1.0
        }};
        let rhs = json! {{
            "type": "number",
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: RangeRemove {
                    removed: Maximum,
                    value: 1.0,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn change_minimum() {
        let lhs = json! {{
            "type": "number",
            "minimum": 1.0
        }};
        let rhs = json! {{
            "type": "number",
            "minimum": 1.3
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: RangeChange {
                    changed: Minimum,
                    old_value: 1.0,
                    new_value: 1.3,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn change_maximum() {
        let lhs = json! {{
            "type": "number",
            "maximum": 1.0
        }};
        let rhs = json! {{
            "type": "number",
            "maximum": 1.3
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: RangeChange {
                    changed: Maximum,
                    old_value: 1.0,
                    new_value: 1.3,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn change_minimum_and_maximum() {
        let lhs = json! {{
            "type": "number",
            "minimum": 1.0,
            "maximum": 2.0,
        }};
        let rhs = json! {{
            "type": "number",
            "minimum": 1.5,
            "maximum": 2.5,
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: RangeChange {
                    changed: Minimum,
                    old_value: 1.0,
                    new_value: 1.5,
                },
            },
            Change {
                path: "",
                change: RangeChange {
                    changed: Maximum,
                    old_value: 2.0,
                    new_value: 2.5,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn unchanged_minimum() {
        let lhs = json! {{
            "type": "number",
            "minimum": 1.3
        }};
        let rhs = json! {{
            "type": "number",
            "minimum": 1.3
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @"[]"
        );
    }

    #[test]
    fn drop_required() {
        let lhs = json! {{
            "required": ["value"]
        }};
        let rhs = json! {{
            "required": [],
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: RequiredRemove {
                    property: "value",
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn add_required() {
        let lhs = json! {{
            "required": []
        }};
        let rhs = json! {{
            "required": ["value"],
        }};

        let diff = diff(lhs, rhs).unwrap();
        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: RequiredAdd {
                    property: "value",
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn any_of_to_equivalent_type() {
        let lhs = json! {{
            "anyOf": [{"type": "string"}],
        }};
        let rhs = json! {{
            "type": "string",
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @"[]"
        );
    }

    #[test]
    fn any_of_to_less_strict_type() {
        let lhs = json! {{
            "anyOf": [{"type": "integer"}, {"type": "string"}],
        }};
        let rhs = json! {{
            "type": "string",
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: TypeRemove {
                    removed: Integer,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn any_of_to_more_strict_type() {
        let lhs = json! {{
            "anyOf": [{"type": "integer"}],
        }};
        let rhs = json! {{
            "type": ["integer", "string"],
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: TypeAdd {
                    added: String,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn any_of_with_constraint_to_type_1() {
        let lhs = json! {{
            "anyOf": [{"type": "integer", "minimum": 1}],
        }};
        let rhs = json! {{
            "type": ["integer", "string"],
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: RangeRemove {
                    removed: Minimum,
                    value: 1.0,
                },
            },
            Change {
                path: "",
                change: TypeAdd {
                    added: String,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn any_of_with_constraint_to_type_2() {
        let lhs = json! {{
            "anyOf": [{"type": "integer", "minimum": 1}, {"type": "integer", "minimum": 2}],
        }};
        let rhs = json! {{
            "type": ["integer", "string"],
            "minimum": 1,
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: RangeRemove {
                    removed: Minimum,
                    value: 2.0,
                },
            },
            Change {
                path: "",
                change: TypeAdd {
                    added: String,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn type_to_equivalent_any_of() {
        let lhs = json! {{
            "type": "integer",
        }};
        let rhs = json! {{
            "anyOf": [{"type": "integer"}],
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @"[]"
        );
    }

    #[test]
    fn type_to_more_strict_any_of() {
        let lhs = json! {{
            "type": "integer",
        }};
        let rhs = json! {{
            "anyOf": [{"type": "integer"}, {"type": "string"}],
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: TypeAdd {
                    added: String,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn type_to_less_strict_any_of() {
        let lhs = json! {{
            "type": "integer",
            "minimum": 1.0
        }};
        let rhs = json! {{
            "anyOf": [{"type": "integer"}],
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(
            diff,
            @r###"
        [
            Change {
                path: "",
                change: RangeRemove {
                    removed: Minimum,
                    value: 1.0,
                },
            },
        ]
        "###
        );
    }

    #[test]
    fn type_to_any_of_within_array() {
        let lhs = json! {{
            "type": "array",
            "items": [
                {"const": "start_unmerge"},
                {"$ref": "#/definitions/Hello"}
            ],
            "definitions": {
                "Hello": {
                    "type": "number",
                }
            }
        }};

        let rhs = json! {{
            "type": "array",
            "items": [
                {"const": "start_unmerge"},
                {"$ref": "#/definitions/Hello"}
            ],
            "definitions": {
                "Hello": {
                    "anyOf": [{"type": "number", "minimum": 1.0}]
                }
            }
        }};

        let diff = diff(lhs, rhs).unwrap();

        assert_debug_snapshot!(diff, @r###"
        [
            Change {
                path: ".1",
                change: RangeAdd {
                    added: Minimum,
                    value: 1.0,
                },
            },
        ]
        "###);
    }
}
