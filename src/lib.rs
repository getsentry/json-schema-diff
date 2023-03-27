use std::collections::{BTreeSet, BTreeMap};

use jsonref::JsonRef;
use serde::{Deserializer, Deserialize, de::Error as _};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Eq, PartialEq)]
pub struct Change {
    path: String,
    change: ChangeInner,
}


#[derive(Debug, Eq, PartialEq)]
pub enum ChangeInner {
    TypeAdd { added: SimpleJsonSchemaType },
    TypeRemove { removed: SimpleJsonSchemaType },
    PropertyAdd { lhs_additional_properties: bool, added: String },
    PropertyRemove { lhs_additional_properties: bool, removed: String },
}

impl ChangeInner {
    pub fn is_breaking(&self) -> bool {
        match self {
            Self::TypeAdd { .. } => false,
            Self::TypeRemove { .. } => true,
            Self::PropertyAdd { lhs_additional_properties, .. } => *lhs_additional_properties,
            Self::PropertyRemove { lhs_additional_properties, .. } => *lhs_additional_properties,
        }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to find references")]
    JsonRef(#[from] jsonref::Error),
    #[error("failed to parse schema")]
    Serde(#[from] serde_json::Error),
}

#[derive(Deserialize, Clone, Ord, Eq, PartialEq, PartialOrd, Debug)]
#[serde(untagged)]
pub enum JsonSchemaType {
    Simple(SimpleJsonSchemaType),
    Multiple(Vec<SimpleJsonSchemaType>),
}

#[derive(Deserialize, Clone, Ord, Eq, PartialEq, PartialOrd, Debug)]
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
    #[serde(skip)]
    Any,
    #[serde(skip)]
    Never,
}

impl SimpleJsonSchemaType {
    fn explode(self) -> Vec<Self> {
        match self {
            Self::Number => vec![Self::Integer, Self::Number],
            Self::Any => vec![Self::String, Self::Number, Self::Integer, Self::Object, Self::Array, Self::Boolean, Self::Null],
            x => vec![x],
        }
    }
}

impl From<SimpleJsonSchemaType> for JsonSchemaType {
    fn from(other: SimpleJsonSchemaType) -> Self {
        JsonSchemaType::Simple(other)
    }
}

impl JsonSchemaType {
    fn as_set(self) -> BTreeSet<SimpleJsonSchemaType> {
        match self {
            JsonSchemaType::Multiple(v) => v
                .into_iter()
                .flat_map(SimpleJsonSchemaType::explode)
                .collect(),
            JsonSchemaType::Simple(x) => vec![x]
                .into_iter()
                .flat_map(SimpleJsonSchemaType::explode)
                .collect(),
        }
    }
}


#[derive(Default, Eq, PartialEq)]
struct JsonSchema {
    ty: Option<JsonSchemaType>,
    constant: Option<Value>,
    additional_properties: Option<Box<JsonSchema>>,
    properties: BTreeMap<String, JsonSchema>,
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
                    ty: Some(SimpleJsonSchemaType::Any.into()),
                    ..Default::default()
                })
            } else {
                Ok(JsonSchema {
                    ty: Some(SimpleJsonSchemaType::Never.into()),
                    ..Default::default()
                })
            }
        } else {
            Ok(JsonSchemaRaw::deserialize(value).map_err(D::Error::custom)?.into())
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
struct JsonSchemaRaw{
    #[serde(rename = "type")]
    ty: Option<JsonSchemaType>,
    #[serde(rename = "const")]
    constant: Option<Value>,
    additional_properties: Option<Box<JsonSchema>>,
    properties: BTreeMap<String, JsonSchema>,
}

impl From<JsonSchemaRaw> for JsonSchema {
    fn from(raw: JsonSchemaRaw) -> Self {
        let JsonSchemaRaw { ty, constant, additional_properties, properties } = raw;
        JsonSchema {
            ty, constant, additional_properties, properties
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
            SimpleJsonSchemaType::Any.into()
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
        .into()
    }
}

pub fn diff(mut lhs: Value, mut rhs: Value) -> Result<Vec<Change>, Error> {
    let mut jsonref = JsonRef::new();
    jsonref.deref_value(&mut lhs)?;
    jsonref.deref_value(&mut rhs)?;
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
    let lhs_ty = lhs.effective_type().as_set();
    let rhs_ty = rhs.effective_type().as_set();

    for removed in lhs_ty.difference(&rhs_ty) {
        rv.push(Change {
            path: json_path.clone(),
            change: ChangeInner::TypeRemove {
                removed: removed.clone().into(),
            },
        });
    }

    for added in rhs_ty.difference(&lhs_ty) {
        rv.push(Change {
            path: json_path.clone(),
            change: ChangeInner::TypeAdd {
                added: added.clone().into(),
            },
        });
    }

    let lhs_props: BTreeSet<_> = lhs.properties.keys().collect();
    let rhs_props: BTreeSet<_> = rhs.properties.keys().collect();

    for &removed in lhs_props.difference(&rhs_props) {
        rv.push(Change {
            path: json_path.clone(),
            change: ChangeInner::PropertyRemove {
                lhs_additional_properties: lhs.additional_properties.as_deref().map_or(true, JsonSchema::is_true),
                removed: removed.to_owned(),
            }
        });
    }

    for &added in rhs_props.difference(&lhs_props) {
        rv.push(Change {
            path: json_path.clone(),
            change: ChangeInner::PropertyAdd {
                lhs_additional_properties: lhs.additional_properties.as_deref().map_or(true, JsonSchema::is_true),
                added: added.to_owned(),
            }
        });
    }

    for common in rhs_props.intersection(&lhs_props) {
        let lhs_child = lhs.properties.get(common.as_str()).unwrap();
        let rhs_child = rhs.properties.get(common.as_str()).unwrap();

        let mut new_path = json_path.clone();
        new_path.push('.');
        new_path.push_str(&common);

        diff_inner(rv, new_path, &lhs_child, &rhs_child)?;
    }

    if let (Some(ref lhs_additional_properties), Some(ref rhs_additional_properties)) = (&lhs.additional_properties, &rhs.additional_properties) {
        if rhs_additional_properties != lhs_additional_properties {
            let mut new_path = json_path.clone();
            new_path.push_str(".<additional properties>");

            diff_inner(rv, new_path, &lhs_additional_properties, &rhs_additional_properties)?;
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
                path: ".<additional properties>",
                change: TypeRemove {
                    removed: Never,
                },
            },
            Change {
                path: ".<additional properties>",
                change: TypeAdd {
                    added: String,
                },
            },
            Change {
                path: ".<additional properties>",
                change: TypeAdd {
                    added: Number,
                },
            },
            Change {
                path: ".<additional properties>",
                change: TypeAdd {
                    added: Integer,
                },
            },
            Change {
                path: ".<additional properties>",
                change: TypeAdd {
                    added: Object,
                },
            },
            Change {
                path: ".<additional properties>",
                change: TypeAdd {
                    added: Array,
                },
            },
            Change {
                path: ".<additional properties>",
                change: TypeAdd {
                    added: Boolean,
                },
            },
            Change {
                path: ".<additional properties>",
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
                path: ".<additional properties>",
                change: TypeRemove {
                    removed: String,
                },
            },
            Change {
                path: ".<additional properties>",
                change: TypeRemove {
                    removed: Number,
                },
            },
            Change {
                path: ".<additional properties>",
                change: TypeRemove {
                    removed: Integer,
                },
            },
            Change {
                path: ".<additional properties>",
                change: TypeRemove {
                    removed: Object,
                },
            },
            Change {
                path: ".<additional properties>",
                change: TypeRemove {
                    removed: Array,
                },
            },
            Change {
                path: ".<additional properties>",
                change: TypeRemove {
                    removed: Boolean,
                },
            },
            Change {
                path: ".<additional properties>",
                change: TypeRemove {
                    removed: Null,
                },
            },
            Change {
                path: ".<additional properties>",
                change: TypeAdd {
                    added: Never,
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
}
