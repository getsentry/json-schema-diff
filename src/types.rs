use schemars::schema::InstanceType;
use serde::Serialize;
use thiserror::Error;

/// An "atomic" change made to the JSON schema in question, going from LHS to RHS.
///
/// Just a wrapper container for `ChangeKind`
#[derive(Debug, PartialEq, Serialize)]
pub struct Change {
    /// JSON path for the given change. `""` for "root schema". `".foo"` for property foo.
    pub path: String,
    /// Data specific to the kind of change.
    pub change: ChangeKind,
}

/// The kind of change + data relevant to the change.
#[derive(Debug, PartialEq, Serialize)]
pub enum ChangeKind {
    /// A type has been added and is now additionally allowed.
    TypeAdd {
        /// The type in question.
        added: JsonSchemaType,
    },
    /// A type has been removed and is no longer allowed.
    TypeRemove {
        /// The type in question.
        removed: JsonSchemaType,
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
    /// A minimum/maximum constraint has been added.
    RangeAdd {
        /// The name of the added constraint.
        added: Range,
        /// The value of the added constraint.
        value: f64,
    },
    /// A minimum/maximum constraint has been removed.
    RangeRemove {
        /// The name of the removed constraint.
        removed: Range,
        /// The value of the removed constraint.
        value: f64,
    },
    /// A minimum/maximum constraint has been updated.
    RangeChange {
        /// The name of the changed constraint.
        changed: Range,
        /// The old constraint value.
        old_value: f64,
        /// The new constraint value.
        new_value: f64,
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
    /// A previously required property has been removed
    RequiredRemove {
        /// The property that is no longer required
        property: String,
    },
    /// A previously optional property has been made required
    RequiredAdd {
        /// The property that is now required
        property: String,
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
            Self::RangeAdd { .. } => true,
            Self::RangeRemove { .. } => false,
            Self::RangeChange {
                changed,
                old_value: lhs_value,
                new_value: rhs_value,
            } => match changed {
                Range::Minimum if lhs_value < rhs_value => true,
                Range::Maximum if lhs_value > rhs_value => true,
                _ => false,
            },
            Self::TupleToArray { .. } => false,
            Self::ArrayToTuple { .. } => true,
            Self::TupleChange { .. } => true,
            Self::RequiredRemove { .. } => false,
            Self::RequiredAdd { .. } => true,
        }
    }
}

/// The errors that can happen in this crate.
#[derive(Error, Debug)]
pub enum Error {
    /// Failed to parse the JSON schema.
    ///
    /// Any deserialization errors from serde that happen while converting the value into our AST
    /// end up here.
    #[error("failed to parse schema")]
    Serde(#[from] serde_json::Error),
}

/// All primitive types defined in JSON schema.
#[derive(Serialize, Clone, Ord, Eq, PartialEq, PartialOrd, Debug)]
#[allow(missing_docs)]
pub enum JsonSchemaType {
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

impl From<JsonSchemaType> for InstanceType {
    fn from(t: JsonSchemaType) -> Self {
        match t {
            JsonSchemaType::String => InstanceType::String,
            JsonSchemaType::Number => InstanceType::Number,
            JsonSchemaType::Integer => InstanceType::Integer,
            JsonSchemaType::Object => InstanceType::Object,
            JsonSchemaType::Array => InstanceType::Array,
            JsonSchemaType::Boolean => InstanceType::Boolean,
            JsonSchemaType::Null => InstanceType::Null,
        }
    }
}

impl From<InstanceType> for JsonSchemaType {
    fn from(t: InstanceType) -> Self {
        match t {
            InstanceType::String => JsonSchemaType::String,
            InstanceType::Number => JsonSchemaType::Number,
            InstanceType::Integer => JsonSchemaType::Integer,
            InstanceType::Object => JsonSchemaType::Object,
            InstanceType::Array => JsonSchemaType::Array,
            InstanceType::Boolean => JsonSchemaType::Boolean,
            InstanceType::Null => JsonSchemaType::Null,
        }
    }
}

/// Range constraints in JSON schema.
#[derive(Serialize, Clone, Ord, Eq, PartialEq, PartialOrd, Debug)]
#[allow(missing_docs)]
pub enum Range {
    #[serde(rename = "minimum")]
    Minimum,
    #[serde(rename = "maximum")]
    Maximum,
}
