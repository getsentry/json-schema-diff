#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
use std::collections::{BTreeMap, BTreeSet};

use jsonref::JsonRef;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use thiserror::Error;

/// An "atomic" change made to the JSON schema in question, going from LHS to RHS.
///
/// Just a wrapper container for `ChangeKind`
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct Change {
    /// JSON path for the given change. `""` for "root schema". `".foo"` for property foo.
    pub path: String,
    /// Data specific to the kind of change.
    pub change: ChangeKind,
}

/// The kind of change + data relevant to the change.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub enum ChangeKind {
    /// A type has been added and is now additionally allowed.
    TypeAdd {
        /// The type in question.
        added: SimpleJsonSchemaType,
    },
    /// A type has been removed and is no longer allowed.
    TypeRemove {
        /// The type in question.
        removed: SimpleJsonSchemaType,
    },
    /// A property has been added and (depending on additionalProperties) is now additionally
    /// allowed.
    PropertyAdd {
        /// The value of additionalProperties within the current JSON object.
        lhs_additional_properties: bool,
        /// The name of the added property.
        added: String,
    },
    /// A property has been removed and (depending on additionalProperties) might now no longer be
    /// allowed.
    PropertyRemove {
        /// The value of additionalProperties within the current JSON object.
        lhs_additional_properties: bool,
        /// The name of the added property.
        removed: String,
    },
    /// An array-type item has been changed from tuple validation to array validation.
    ///
    /// See https://json-schema.org/understanding-json-schema/reference/array.html
    ///
    /// Changes will still be emitted for inner items.
    TupleToArray {
        /// The length of the (old) tuple
        old_length: usize,
    },
    /// An array-type item has been changed from array validation to tuple validation.
    ///
    /// See https://json-schema.org/understanding-json-schema/reference/array.html
    ///
    /// Changes will still be emitted for inner items.
    ArrayToTuple {
        /// The length of the (new) tuple
        new_length: usize,
    },
    /// An array-type item with tuple validation has changed its length ("items" array got longer
    /// or shorter.
    ///
    /// See https://json-schema.org/understanding-json-schema/reference/array.html
    ///
    /// Changes will still be emitted for inner items.
    TupleChange {
        /// The new length of the tuple
        new_length: usize,
    },
}

impl ChangeKind {
    /// Whether the change is breaking.
    ///
    /// What is considered breaking is WIP. Changes are intentionally exposed as-is in public API
    /// so that the user can develop their own logic as to what they consider breaking.
    ///
    /// Currently the rule of thumb is, a change is breaking if it would cause messages that used
    /// to validate fine under RHS to no longer validate under LHS.
    pub fn is_breaking(&self) -> bool {
        match self {
            Self::TypeAdd { .. } => false,
            Self::TypeRemove { .. } => true,
            Self::PropertyAdd {
                lhs_additional_properties,
                ..
            } => *lhs_additional_properties,
            Self::PropertyRemove {
                lhs_additional_properties,
                ..
            } => !*lhs_additional_properties,
            Self::TupleToArray { .. } => false,
            Self::ArrayToTuple { .. } => true,
            Self::TupleChange { .. } => true,
        }
    }
}

/// The errors that can happen in this crate.
#[derive(Error, Debug)]
pub enum Error {
    /// Failed to resolve references upfront.
    ///
    /// We invoke the `jsonref` crate upfront to flatten out the schema. Any errors during that
    /// stage end up here.
    #[error("failed to find references")]
    JsonRef(#[from] Box<jsonref::Error>),

    /// Failed to parse the JSON schema.
    ///
    /// Any deserialization errors from serde that happen while converting the value into our AST
    /// end up here.
    #[error("failed to parse schema")]
    Serde(#[from] serde_json::Error),
}

#[derive(Deserialize, Clone, Ord, Eq, PartialEq, PartialOrd, Debug)]
#[serde(untagged)]
enum JsonSchemaType {
    Simple(SimpleJsonSchemaType),
    Any,
    Never,
    Multiple(Vec<SimpleJsonSchemaType>),
}

/// All primitive types defined in JSON schema.
#[derive(Serialize, Deserialize, Clone, Ord, Eq, PartialEq, PartialOrd, Debug)]
#[allow(missing_docs)]
pub enum SimpleJsonSchemaType {
    #[serde(rename = "string")]
    String,
    #[serde(rename = "number")]
    Number,
    #[serde(rename = "integer")]
    Integer,
    #[serde(rename = "object")]
    Object,
    #[serde(rename = "array")]
    Array,
    #[serde(rename = "boolean")]
    Boolean,
    #[serde(rename = "null")]
    Null,
}

impl From<SimpleJsonSchemaType> for JsonSchemaType {
    fn from(other: SimpleJsonSchemaType) -> Self {
        JsonSchemaType::Simple(other)
    }
}

impl JsonSchemaType {
    fn into_set(self) -> BTreeSet<SimpleJsonSchemaType> {
        self.explode().into_iter().collect()
    }

    fn explode(self) -> Vec<SimpleJsonSchemaType> {
        match self {
            Self::Simple(SimpleJsonSchemaType::Number) => {
                vec![SimpleJsonSchemaType::Integer, SimpleJsonSchemaType::Number]
            }
            Self::Any => vec![
                SimpleJsonSchemaType::String,
                SimpleJsonSchemaType::Number,
                SimpleJsonSchemaType::Integer,
                SimpleJsonSchemaType::Object,
                SimpleJsonSchemaType::Array,
                SimpleJsonSchemaType::Boolean,
                SimpleJsonSchemaType::Null,
            ],
            Self::Never => vec![],
            Self::Simple(x) => vec![x],
            Self::Multiple(xs) => xs
                .into_iter()
                .map(JsonSchemaType::from)
                .flat_map(Self::explode)
                .collect(),
        }
    }
}

#[derive(Default, Eq, PartialEq, Debug)]
struct JsonSchema {
    ty: Option<JsonSchemaType>,
    constant: Option<Value>,
    additional_properties: Option<Box<JsonSchema>>,
    properties: BTreeMap<String, JsonSchema>,
    any_of: Vec<JsonSchema>,
    items: Option<JsonSchemaItems>,
}

impl JsonSchema {
    fn is_true(&self) -> bool {
        *self == JsonSchema::default()
    }
}

impl<'de> Deserialize<'de> for JsonSchema {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // perhaps catch this error as well, if needed
        let value = Value::deserialize(deserializer)?;
        if let Value::Bool(boolean) = value {
            if boolean {
                Ok(JsonSchema {
                    ty: Some(JsonSchemaType::Any),
                    ..Default::default()
                })
            } else {
                Ok(JsonSchema {
                    ty: Some(JsonSchemaType::Never),
                    ..Default::default()
                })
            }
        } else {
            Ok(JsonSchemaRaw::deserialize(value)
                .map_err(D::Error::custom)?
                .into())
        }
    }
}

#[derive(Deserialize, Eq, PartialEq, Debug)]
#[serde(untagged)]
enum JsonSchemaItems {
    ExactItems(Vec<JsonSchema>),
    Items(Box<JsonSchema>),
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, serde(deny_unknown_fields))]
#[serde(default)]
struct JsonSchemaRaw {
    #[serde(rename = "type")]
    ty: Option<JsonSchemaType>,
    #[serde(rename = "const")]
    constant: Option<Value>,
    additional_properties: Option<Box<JsonSchema>>,
    properties: BTreeMap<String, JsonSchema>,
    any_of: Vec<JsonSchema>,
    items: Option<JsonSchemaItems>,
}

impl From<JsonSchemaRaw> for JsonSchema {
    fn from(raw: JsonSchemaRaw) -> Self {
        let JsonSchemaRaw {
            ty,
            constant,
            additional_properties,
            properties,
            any_of,
            items,
        } = raw;
        JsonSchema {
            ty,
            constant,
            additional_properties,
            properties,
            any_of,
            items,
        }
    }
}

impl JsonSchema {
    fn effective_type(&self) -> JsonSchemaType {
        if let Some(ref ty) = self.ty {
            ty.clone()
        } else if let Some(ref constant) = self.constant {
            SimpleJsonSchemaType::from(constant).into()
        } else if !self.properties.is_empty() {
            SimpleJsonSchemaType::Object.into()
        } else {
            JsonSchemaType::Any
        }
    }
}

impl From<&Value> for SimpleJsonSchemaType {
    fn from(val: &Value) -> SimpleJsonSchemaType {
        match val {
            Value::Number(_) => SimpleJsonSchemaType::Number,
            Value::Null => SimpleJsonSchemaType::Null,
            Value::String(_) => SimpleJsonSchemaType::String,
            Value::Bool(_) => SimpleJsonSchemaType::Boolean,
            Value::Array(_) => SimpleJsonSchemaType::Array,
            Value::Object(_) => SimpleJsonSchemaType::Object,
        }
    }
}

/// Take two JSON schemas, and compare them.
///
/// `lhs` (left-hand side) is the old schema, `rhs` (right-hand side) is the new schema.
pub fn diff(mut lhs: Value, mut rhs: Value) -> Result<Vec<Change>, Error> {
    let mut jsonref = JsonRef::new();
    jsonref.deref_value(&mut lhs).map_err(Box::new)?;
    jsonref.deref_value(&mut rhs).map_err(Box::new)?;
    let mut rv = Vec::new();

    let json_path = String::new();

    let lhs: JsonSchema = serde_json::from_value(lhs)?;
    let rhs: JsonSchema = serde_json::from_value(rhs)?;

    diff_inner(&mut rv, json_path, &lhs, &rhs)?;

    Ok(rv)
}

fn diff_inner(
    rv: &mut Vec<Change>,
    json_path: String,
    lhs: &JsonSchema,
    rhs: &JsonSchema,
) -> Result<(), Error> {
    #[cfg(test)]
    {
        dbg!(&json_path);
        dbg!(&rhs);
        dbg!(&lhs);
    }

    for (i, (lhs_inner, rhs_inner)) in lhs.any_of.iter().zip(rhs.any_of.iter()).enumerate() {
        let new_path = format!("{json_path}.<anyOf:{i}>");
        diff_inner(rv, new_path, lhs_inner, rhs_inner)?;
    }

    let lhs_ty = lhs.effective_type().into_set();
    let rhs_ty = rhs.effective_type().into_set();

    for removed in lhs_ty.difference(&rhs_ty) {
        rv.push(Change {
            path: json_path.clone(),
            change: ChangeKind::TypeRemove {
                removed: removed.clone(),
            },
        });
    }

    for added in rhs_ty.difference(&lhs_ty) {
        rv.push(Change {
            path: json_path.clone(),
            change: ChangeKind::TypeAdd {
                added: added.clone(),
            },
        });
    }

    let lhs_props: BTreeSet<_> = lhs.properties.keys().collect();
    let rhs_props: BTreeSet<_> = rhs.properties.keys().collect();

    for &removed in lhs_props.difference(&rhs_props) {
        rv.push(Change {
            path: json_path.clone(),
            change: ChangeKind::PropertyRemove {
                lhs_additional_properties: lhs
                    .additional_properties
                    .as_deref()
                    .map_or(true, JsonSchema::is_true),
                removed: removed.to_owned(),
            },
        });
    }

    for &added in rhs_props.difference(&lhs_props) {
        rv.push(Change {
            path: json_path.clone(),
            change: ChangeKind::PropertyAdd {
                lhs_additional_properties: lhs
                    .additional_properties
                    .as_deref()
                    .map_or(true, JsonSchema::is_true),
                added: added.to_owned(),
            },
        });
    }

    for common in rhs_props.intersection(&lhs_props) {
        let lhs_child = lhs.properties.get(common.as_str()).unwrap();
        let rhs_child = rhs.properties.get(common.as_str()).unwrap();

        let new_path = format!("{json_path}.{common}");
        diff_inner(rv, new_path, lhs_child, rhs_child)?;
    }

    if let (Some(ref lhs_additional_properties), Some(ref rhs_additional_properties)) =
        (&lhs.additional_properties, &rhs.additional_properties)
    {
        if rhs_additional_properties != lhs_additional_properties {
            let new_path = format!("{json_path}.<additionalProperties>");

            diff_inner(
                rv,
                new_path,
                lhs_additional_properties,
                rhs_additional_properties,
            )?;
        }
    }

    match (&lhs.items, &rhs.items) {
        (
            Some(JsonSchemaItems::ExactItems(lhs_items)),
            Some(JsonSchemaItems::ExactItems(rhs_items)),
        ) => {
            if lhs_items.len() != rhs_items.len() {
                rv.push(Change {
                    path: json_path.clone(),
                    change: ChangeKind::TupleChange {
                        new_length: rhs_items.len(),
                    },
                });
            }

            for (i, (lhs_inner, rhs_inner)) in lhs_items.iter().zip(rhs_items.iter()).enumerate() {
                let new_path = format!("{json_path}.{i}");
                diff_inner(rv, new_path, lhs_inner, rhs_inner)?;
            }
        }
        (Some(JsonSchemaItems::Items(lhs_inner)), Some(JsonSchemaItems::Items(rhs_inner))) => {
            let new_path = format!("{json_path}.?");
            diff_inner(rv, new_path, lhs_inner, rhs_inner)?;
        }
        (Some(JsonSchemaItems::Items(lhs_inner)), Some(JsonSchemaItems::ExactItems(rhs_items))) => {
            rv.push(Change {
                path: json_path.clone(),
                change: ChangeKind::ArrayToTuple {
                    new_length: rhs_items.len(),
                },
            });

            for (i, rhs_inner) in rhs_items.iter().enumerate() {
                let new_path = format!("{json_path}.{i}");
                diff_inner(rv, new_path, lhs_inner, rhs_inner)?;
            }
        }
        (Some(JsonSchemaItems::ExactItems(lhs_items)), Some(JsonSchemaItems::Items(rhs_inner))) => {
            rv.push(Change {
                path: json_path.clone(),
                change: ChangeKind::TupleToArray {
                    old_length: lhs_items.len(),
                },
            });

            for (i, lhs_inner) in lhs_items.iter().enumerate() {
                let new_path = format!("{json_path}.{i}");
                diff_inner(rv, new_path, lhs_inner, rhs_inner)?;
            }
        }
        (None, None) => (),
        _ => {
            #[cfg(test)]
            todo!("{:?} {:?}", lhs.items, rhs.items)
        }
    }

    Ok(())
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
}
