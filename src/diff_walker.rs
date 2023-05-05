use std::collections::BTreeSet;

use schemars::schema::{InstanceType, RootSchema, Schema, SchemaObject, SingleOrVec};
use serde_json::Value;

use crate::{Change, ChangeKind, Error, JsonSchemaType, Range};

pub struct DiffWalker {
    pub changes: Vec<Change>,
    pub lhs_root: RootSchema,
    pub rhs_root: RootSchema,
}

impl DiffWalker {
    fn diff_any_of(
        &mut self,
        json_path: &str,
        lhs: &mut SchemaObject,
        rhs: &mut SchemaObject,
    ) -> Result<(), Error> {
        // hack to get a stable order for anyOf. serde_json::Value does not impl Hash or Ord, so we
        // can't use a set.
        if let (Some(lhs_any_of), Some(rhs_any_of)) =
            (&mut lhs.subschemas().any_of, &mut rhs.subschemas().any_of)
        {
            lhs_any_of.sort_by_cached_key(|x| format!("{x:?}"));
            rhs_any_of.sort_by_cached_key(|x| format!("{x:?}"));

            for (i, (lhs_inner, rhs_inner)) in
                lhs_any_of.iter_mut().zip(rhs_any_of.iter_mut()).enumerate()
            {
                let new_path = format!("{json_path}.<anyOf:{i}>");
                self.diff(
                    &new_path,
                    &mut lhs_inner.clone().into_object(),
                    &mut rhs_inner.clone().into_object(),
                )?;
            }
        }

        Ok(())
    }

    fn diff_instance_types(
        &mut self,
        json_path: &str,
        lhs: &mut SchemaObject,
        rhs: &mut SchemaObject,
    ) {
        let lhs_ty = lhs.effective_type().into_set();
        let rhs_ty = rhs.effective_type().into_set();

        for removed in lhs_ty.difference(&rhs_ty) {
            self.changes.push(Change {
                path: json_path.to_owned(),
                change: ChangeKind::TypeRemove {
                    removed: removed.clone(),
                },
            });
        }

        for added in rhs_ty.difference(&lhs_ty) {
            self.changes.push(Change {
                path: json_path.to_owned(),
                change: ChangeKind::TypeAdd {
                    added: added.clone(),
                },
            });
        }
    }

    fn diff_properties(
        &mut self,
        json_path: &str,
        lhs: &mut SchemaObject,
        rhs: &mut SchemaObject,
    ) -> Result<(), Error> {
        let lhs_props: BTreeSet<_> = lhs.object().properties.keys().cloned().collect();
        let rhs_props: BTreeSet<_> = rhs.object().properties.keys().cloned().collect();

        let lhs_additional_properties = lhs
            .object()
            .additional_properties
            .as_ref()
            .map_or(true, |x| x.clone().into_object().is_true());

        for removed in lhs_props.difference(&rhs_props) {
            self.changes.push(Change {
                path: json_path.to_owned(),
                change: ChangeKind::PropertyRemove {
                    lhs_additional_properties,
                    removed: removed.clone(),
                },
            });
        }

        for added in rhs_props.difference(&lhs_props) {
            self.changes.push(Change {
                path: json_path.to_owned(),
                change: ChangeKind::PropertyAdd {
                    lhs_additional_properties,
                    added: added.clone(),
                },
            });
        }

        for common in rhs_props.intersection(&lhs_props) {
            let lhs_child = lhs.object().properties.get(common.as_str()).unwrap();
            let rhs_child = rhs.object().properties.get(common.as_str()).unwrap();

            let new_path = format!("{json_path}.{common}");
            self.diff(
                &new_path,
                &mut lhs_child.clone().into_object(),
                &mut rhs_child.clone().into_object(),
            )?;
        }

        Ok(())
    }

    fn diff_additional_properties(
        &mut self,
        json_path: &str,
        lhs: &mut SchemaObject,
        rhs: &mut SchemaObject,
    ) -> Result<(), Error> {
        if let (Some(ref lhs_additional_properties), Some(ref rhs_additional_properties)) = (
            &lhs.object().additional_properties,
            &rhs.object().additional_properties,
        ) {
            if rhs_additional_properties != lhs_additional_properties {
                let new_path = format!("{json_path}.<additionalProperties>");

                self.diff(
                    &new_path,
                    &mut lhs_additional_properties.clone().into_object(),
                    &mut rhs_additional_properties.clone().into_object(),
                )?;
            }
        }

        Ok(())
    }

    fn diff_range(
        &mut self,
        json_path: &str,
        lhs: &mut SchemaObject,
        rhs: &mut SchemaObject,
    ) -> Result<(), Error> {
        let diff = |lhs, rhs, range| match (lhs, rhs) {
            (None, Some(value)) => Some(Change {
                path: json_path.to_owned(),
                change: ChangeKind::RangeAdd {
                    added: range,
                    value,
                },
            }),
            (Some(value), None) => Some(Change {
                path: json_path.to_owned(),
                change: ChangeKind::RangeRemove {
                    removed: range,
                    value,
                },
            }),
            (Some(lhs), Some(rhs)) if lhs != rhs => Some(Change {
                path: json_path.to_owned(),
                change: ChangeKind::RangeChange {
                    changed: range,
                    old_value: lhs,
                    new_value: rhs,
                },
            }),
            _ => None,
        };
        diff(lhs.number().minimum, rhs.number().minimum, Range::Minimum)
            .map(|diff| self.changes.push(diff));
        diff(lhs.number().maximum, rhs.number().maximum, Range::Maximum)
            .map(|diff| self.changes.push(diff));
        Ok(())
    }

    fn diff_array_items(
        &mut self,
        json_path: &str,
        lhs: &mut SchemaObject,
        rhs: &mut SchemaObject,
    ) -> Result<(), Error> {
        match (&lhs.array().items, &rhs.array().items) {
            (Some(SingleOrVec::Vec(lhs_items)), Some(SingleOrVec::Vec(rhs_items))) => {
                if lhs_items.len() != rhs_items.len() {
                    self.changes.push(Change {
                        path: json_path.to_owned(),
                        change: ChangeKind::TupleChange {
                            new_length: rhs_items.len(),
                        },
                    });
                }

                for (i, (lhs_inner, rhs_inner)) in
                    lhs_items.iter().zip(rhs_items.iter()).enumerate()
                {
                    let new_path = format!("{json_path}.{i}");
                    self.diff(
                        &new_path,
                        &mut lhs_inner.clone().into_object(),
                        &mut rhs_inner.clone().into_object(),
                    )?;
                }
            }
            (Some(SingleOrVec::Single(lhs_inner)), Some(SingleOrVec::Single(rhs_inner))) => {
                let new_path = format!("{json_path}.?");
                self.diff(
                    &new_path,
                    &mut lhs_inner.clone().into_object(),
                    &mut rhs_inner.clone().into_object(),
                )?;
            }
            (Some(SingleOrVec::Single(lhs_inner)), Some(SingleOrVec::Vec(rhs_items))) => {
                self.changes.push(Change {
                    path: json_path.to_owned(),
                    change: ChangeKind::ArrayToTuple {
                        new_length: rhs_items.len(),
                    },
                });

                for (i, rhs_inner) in rhs_items.iter().enumerate() {
                    let new_path = format!("{json_path}.{i}");
                    self.diff(
                        &new_path,
                        &mut lhs_inner.clone().into_object(),
                        &mut rhs_inner.clone().into_object(),
                    )?;
                }
            }
            (Some(SingleOrVec::Vec(lhs_items)), Some(SingleOrVec::Single(rhs_inner))) => {
                self.changes.push(Change {
                    path: json_path.to_owned(),
                    change: ChangeKind::TupleToArray {
                        old_length: lhs_items.len(),
                    },
                });

                for (i, lhs_inner) in lhs_items.iter().enumerate() {
                    let new_path = format!("{json_path}.{i}");
                    self.diff(
                        &new_path,
                        &mut lhs_inner.clone().into_object(),
                        &mut rhs_inner.clone().into_object(),
                    )?;
                }
            }
            (None, None) => (),

            #[cfg(not(test))]
            _ => (),
            #[cfg(test)]
            (x, y) => todo!("{:?} {:?}", x, y),
        }

        Ok(())
    }

    fn resolve_ref<'a>(root_schema: &'a RootSchema, reference: &str) -> Option<&'a Schema> {
        if let Some(definition_name) = reference.strip_prefix("#/definitions/") {
            let schema_object = root_schema.definitions.get(definition_name)?;
            Some(schema_object)
        } else {
            None
        }
    }

    fn resolve_references(
        &mut self,
        lhs: &mut SchemaObject,
        rhs: &mut SchemaObject,
    ) -> Result<(), Error> {
        if let Some(ref reference) = lhs.reference {
            if let Some(lhs_inner) = Self::resolve_ref(&self.lhs_root, reference) {
                *lhs = lhs_inner.clone().into_object();
            }
        }

        if let Some(ref reference) = rhs.reference {
            if let Some(rhs_inner) = Self::resolve_ref(&self.rhs_root, reference) {
                *rhs = rhs_inner.clone().into_object();
            }
        }

        Ok(())
    }

    pub fn diff(
        &mut self,
        json_path: &str,
        lhs: &mut SchemaObject,
        rhs: &mut SchemaObject,
    ) -> Result<(), Error> {
        self.resolve_references(lhs, rhs)?;
        self.diff_any_of(json_path, lhs, rhs)?;
        self.diff_instance_types(json_path, lhs, rhs);
        self.diff_properties(json_path, lhs, rhs)?;
        self.diff_range(json_path, lhs, rhs)?;
        self.diff_additional_properties(json_path, lhs, rhs)?;
        self.diff_array_items(json_path, lhs, rhs)?;
        Ok(())
    }
}

trait JsonSchemaExt {
    fn is_true(&self) -> bool;
    fn effective_type(&mut self) -> InternalJsonSchemaType;
}

impl JsonSchemaExt for SchemaObject {
    fn is_true(&self) -> bool {
        *self == SchemaObject::default()
    }

    fn effective_type(&mut self) -> InternalJsonSchemaType {
        if let Some(ref ty) = self.instance_type {
            match ty {
                SingleOrVec::Single(ty) => schemars_to_own(**ty).into(),
                SingleOrVec::Vec(tys) => InternalJsonSchemaType::Multiple(
                    tys.iter().copied().map(schemars_to_own).collect(),
                ),
            }
        } else if let Some(ref constant) = self.const_value {
            serde_value_to_own(constant).into()
        } else if !self.object().properties.is_empty() {
            JsonSchemaType::Object.into()
        } else if self
            .subschemas()
            .not
            .as_ref()
            .map_or(false, |x| x.clone().into_object().is_true())
        {
            InternalJsonSchemaType::Never
        } else {
            InternalJsonSchemaType::Any
        }
    }
}

#[derive(Clone, Ord, Eq, PartialEq, PartialOrd, Debug)]
enum InternalJsonSchemaType {
    Simple(JsonSchemaType),
    Any,
    Never,
    Multiple(Vec<JsonSchemaType>),
}

impl From<JsonSchemaType> for InternalJsonSchemaType {
    fn from(other: JsonSchemaType) -> Self {
        InternalJsonSchemaType::Simple(other)
    }
}

impl InternalJsonSchemaType {
    fn into_set(self) -> BTreeSet<JsonSchemaType> {
        self.explode().into_iter().collect()
    }

    fn explode(self) -> Vec<JsonSchemaType> {
        match self {
            Self::Simple(JsonSchemaType::Number) => {
                vec![JsonSchemaType::Integer, JsonSchemaType::Number]
            }
            Self::Any => vec![
                JsonSchemaType::String,
                JsonSchemaType::Number,
                JsonSchemaType::Integer,
                JsonSchemaType::Object,
                JsonSchemaType::Array,
                JsonSchemaType::Boolean,
                JsonSchemaType::Null,
            ],
            Self::Never => vec![],
            Self::Simple(x) => vec![x],
            Self::Multiple(xs) => xs
                .into_iter()
                .map(InternalJsonSchemaType::from)
                .flat_map(Self::explode)
                .collect(),
        }
    }
}

fn serde_value_to_own(val: &Value) -> JsonSchemaType {
    match val {
        Value::Number(_) => JsonSchemaType::Number,
        Value::Null => JsonSchemaType::Null,
        Value::String(_) => JsonSchemaType::String,
        Value::Bool(_) => JsonSchemaType::Boolean,
        Value::Array(_) => JsonSchemaType::Array,
        Value::Object(_) => JsonSchemaType::Object,
    }
}

fn schemars_to_own(other: InstanceType) -> JsonSchemaType {
    match other {
        InstanceType::Null => JsonSchemaType::Null,
        InstanceType::Boolean => JsonSchemaType::Boolean,
        InstanceType::Object => JsonSchemaType::Object,
        InstanceType::Array => JsonSchemaType::Array,
        InstanceType::Number => JsonSchemaType::Number,
        InstanceType::String => JsonSchemaType::String,
        InstanceType::Integer => JsonSchemaType::Integer,
    }
}
