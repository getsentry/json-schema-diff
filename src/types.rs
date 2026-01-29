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
    /// A const value has been added as an allowed value.
    ConstAdd {
        /// The value of the added const.
        added: serde_json::Value,
    },
    /// A const has been removed as an allowed value.
    ConstRemove {
        /// The value of the removed const.
        removed: serde_json::Value,
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
        /// The value of the added constraint.
        added: Range,
    },
    /// A minimum/maximum constraint has been removed.
    RangeRemove {
        /// The value of the removed constraint.
        removed: Range,
    },
    /// A minimum/maximum constraint has been updated.
    RangeChange {
        /// The old constraint value.
        old_value: Range,
        /// The new constraint value.
        new_value: Range,
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
    /// A format constraint has been added.
    FormatAdd {
        /// The format that was added.
        added: String,
    },
    /// A format constraint has been removed.
    FormatRemove {
        /// The format that was removed.
        removed: String,
    },
    /// A format constraint has been changed.
    FormatChange {
        /// The old format value.
        old_format: String,
        /// The new format value.
        new_format: String,
    },
    /// A pattern constraint has been added.
    PatternAdd {
        /// The pattern that was added.
        added: String,
    },
    /// A pattern constraint has been removed.
    PatternRemove {
        /// The pattern that was removed.
        removed: String,
    },
    /// A pattern constraint has been changed.
    PatternChange {
        /// The old pattern value.
        old_pattern: String,
        /// The new pattern value.
        new_pattern: String,
    },
    /// A minLength constraint has been added.
    MinLengthAdd {
        /// The minLength value that was added.
        added: u32,
    },
    /// A minLength constraint has been removed.
    MinLengthRemove {
        /// The minLength value that was removed.
        removed: u32,
    },
    /// A minLength constraint has been changed.
    MinLengthChange {
        /// The old minLength value.
        old_value: u32,
        /// The new minLength value.
        new_value: u32,
    },
    /// A maxLength constraint has been added.
    MaxLengthAdd {
        /// The maxLength value that was added.
        added: u32,
    },
    /// A maxLength constraint has been removed.
    MaxLengthRemove {
        /// The maxLength value that was removed.
        removed: u32,
    },
    /// A maxLength constraint has been changed.
    MaxLengthChange {
        /// The old maxLength value.
        old_value: u32,
        /// The new maxLength value.
        new_value: u32,
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
            Self::ConstAdd { .. } => true,
            Self::ConstRemove { .. } => false,
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
                old_value,
                new_value,
            } => match (old_value, new_value) {
                (Range::ExclusiveMinimum(exc), Range::Minimum(min)) if exc >= min => false,
                (Range::ExclusiveMaximum(exc), Range::Maximum(max)) if exc <= max => false,
                (Range::Minimum(l), Range::Minimum(r)) if l >= r => false,
                (Range::ExclusiveMinimum(l), Range::ExclusiveMinimum(r)) if l >= r => false,
                (Range::Maximum(l), Range::Maximum(r)) if l <= r => false,
                (Range::ExclusiveMaximum(l), Range::ExclusiveMaximum(r)) if l <= r => false,
                _ => true,
            },
            Self::TupleToArray { .. } => false,
            Self::ArrayToTuple { .. } => true,
            Self::TupleChange { .. } => true,
            Self::RequiredRemove { .. } => false,
            Self::RequiredAdd { .. } => true,
            Self::FormatAdd { .. } => true,
            Self::FormatRemove { .. } => false,
            Self::FormatChange { .. } => true,
            // Pattern changes are conservatively treated as breaking.
            // Determining if one regex is a subset of another requires complex analysis.
            Self::PatternAdd { .. } => true,
            Self::PatternRemove { .. } => false,
            Self::PatternChange { .. } => true,
            // MinLength: increasing restricts (breaking), decreasing relaxes (non-breaking)
            Self::MinLengthAdd { .. } => true,
            Self::MinLengthRemove { .. } => false,
            Self::MinLengthChange {
                old_value,
                new_value,
            } => new_value > old_value,
            // MaxLength: decreasing restricts (breaking), increasing relaxes (non-breaking)
            Self::MaxLengthAdd { .. } => true,
            Self::MaxLengthRemove { .. } => false,
            Self::MaxLengthChange {
                old_value,
                new_value,
            } => new_value < old_value,
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
#[derive(Serialize, Clone, PartialEq, PartialOrd, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)]
pub enum Range {
    Minimum(f64),
    Maximum(f64),
    ExclusiveMinimum(f64),
    ExclusiveMaximum(f64),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn is_range_change_breaking() {
        assert!(!ChangeKind::RangeChange {
            old_value: Range::Minimum(1.0),
            new_value: Range::Minimum(1.0),
        }
        .is_breaking());

        assert!(ChangeKind::RangeChange {
            old_value: Range::Minimum(1.0),
            new_value: Range::Minimum(2.0),
        }
        .is_breaking());

        assert!(!ChangeKind::RangeChange {
            old_value: Range::Minimum(2.0),
            new_value: Range::Minimum(1.0),
        }
        .is_breaking());

        assert!(ChangeKind::RangeChange {
            old_value: Range::Minimum(1.0),
            new_value: Range::ExclusiveMinimum(1.0),
        }
        .is_breaking());

        assert!(ChangeKind::RangeChange {
            old_value: Range::Minimum(1.0),
            new_value: Range::ExclusiveMinimum(2.0),
        }
        .is_breaking());

        assert!(ChangeKind::RangeChange {
            old_value: Range::Minimum(2.0),
            new_value: Range::ExclusiveMinimum(1.0),
        }
        .is_breaking());

        assert!(!ChangeKind::RangeChange {
            old_value: Range::ExclusiveMinimum(1.0),
            new_value: Range::ExclusiveMinimum(1.0),
        }
        .is_breaking());

        assert!(ChangeKind::RangeChange {
            old_value: Range::ExclusiveMinimum(1.0),
            new_value: Range::ExclusiveMinimum(2.0),
        }
        .is_breaking());

        assert!(!ChangeKind::RangeChange {
            old_value: Range::ExclusiveMinimum(2.0),
            new_value: Range::ExclusiveMinimum(1.0),
        }
        .is_breaking());

        assert!(!ChangeKind::RangeChange {
            old_value: Range::Maximum(1.0),
            new_value: Range::Maximum(1.0),
        }
        .is_breaking());

        assert!(!ChangeKind::RangeChange {
            old_value: Range::Maximum(1.0),
            new_value: Range::Maximum(2.0),
        }
        .is_breaking());

        assert!(ChangeKind::RangeChange {
            old_value: Range::Maximum(2.0),
            new_value: Range::Maximum(1.0),
        }
        .is_breaking());

        assert!(ChangeKind::RangeChange {
            old_value: Range::Maximum(1.0),
            new_value: Range::ExclusiveMaximum(1.0),
        }
        .is_breaking());

        assert!(ChangeKind::RangeChange {
            old_value: Range::Maximum(1.0),
            new_value: Range::ExclusiveMaximum(2.0),
        }
        .is_breaking());

        assert!(ChangeKind::RangeChange {
            old_value: Range::Maximum(2.0),
            new_value: Range::ExclusiveMaximum(1.0),
        }
        .is_breaking());

        assert!(!ChangeKind::RangeChange {
            old_value: Range::ExclusiveMaximum(1.0),
            new_value: Range::ExclusiveMaximum(1.0),
        }
        .is_breaking());

        assert!(!ChangeKind::RangeChange {
            old_value: Range::ExclusiveMaximum(1.0),
            new_value: Range::ExclusiveMaximum(2.0),
        }
        .is_breaking());

        assert!(ChangeKind::RangeChange {
            old_value: Range::ExclusiveMaximum(2.0),
            new_value: Range::ExclusiveMaximum(1.0),
        }
        .is_breaking());
    }
}
