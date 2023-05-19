use std::collections::{BTreeMap, BTreeSet};

use schemars::schema::{
    InstanceType, NumberValidation, ObjectValidation, RootSchema, Schema, SchemaObject,
    SingleOrVec, SubschemaValidation,
};
use serde_json::Value;

use crate::{Change, ChangeKind, Error, JsonSchemaType, Range};

pub struct DiffWalker<F: FnMut(Change)> {
    pub cb: F,
    pub lhs_root: RootSchema,
    pub rhs_root: RootSchema,
}

impl<F: FnMut(Change)> DiffWalker<F> {
    pub fn new(cb: F, lhs_root: RootSchema, rhs_root: RootSchema) -> Self {
        Self {
            cb,
            lhs_root,
            rhs_root,
        }
    }

    fn diff_any_of(
        &mut self,
        json_path: &str,
        is_rhs_split: bool,
        lhs: &mut SchemaObject,
        rhs: &mut SchemaObject,
    ) -> Result<(), Error> {
        // hack to get a stable order for anyOf. serde_json::Value does not impl Hash or Ord, so we
        // can't use a set.
        if let (Some(lhs_any_of), Some(rhs_any_of)) =
            (&mut lhs.subschemas().any_of, &mut rhs.subschemas().any_of)
        {
            let max_len = lhs_any_of.len().max(rhs_any_of.len());
            lhs_any_of.resize(max_len, Schema::Bool(false));
            rhs_any_of.resize(max_len, Schema::Bool(false));

            let mut mat = pathfinding::matrix::Matrix::new(max_len, max_len, 0i32);
            for (i, l) in lhs_any_of.iter_mut().enumerate() {
                for (j, r) in rhs_any_of.iter_mut().enumerate() {
                    let mut count = 0;
                    let counter = |_change: Change| count += 1;
                    DiffWalker::new(
                        Box::new(counter) as Box<dyn FnMut(Change)>,
                        self.lhs_root.clone(),
                        self.rhs_root.clone(),
                    )
                    .diff("", l, r)?;
                    mat[(i, j)] = count;
                }
            }
            let pairs = pathfinding::kuhn_munkres::kuhn_munkres_min(&mat).1;
            for i in 0..max_len {
                let new_path = match is_rhs_split {
                    true => json_path.to_owned(),
                    false => format!("{json_path}.<anyOf:{}>", pairs[i]),
                };
                self.do_diff(
                    &new_path,
                    true,
                    &mut lhs_any_of[i].clone().into_object(),
                    &mut rhs_any_of[pairs[i]].clone().into_object(),
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
            (self.cb)(Change {
                path: json_path.to_owned(),
                change: ChangeKind::TypeRemove {
                    removed: removed.clone(),
                },
            });
        }

        for added in rhs_ty.difference(&lhs_ty) {
            (self.cb)(Change {
                path: json_path.to_owned(),
                change: ChangeKind::TypeAdd {
                    added: added.clone(),
                },
            });
        }
    }

    fn diff_const(&mut self, json_path: &str, lhs: &mut SchemaObject, rhs: &mut SchemaObject) {
        Self::normalize_const(lhs);
        Self::normalize_const(rhs);
        match (&lhs.const_value, &rhs.const_value) {
            (Some(value), None) => (self.cb)(Change {
                path: json_path.to_owned(),
                change: ChangeKind::ConstRemove {
                    removed: value.clone(),
                },
            }),
            (None, Some(value)) => (self.cb)(Change {
                path: json_path.to_owned(),
                change: ChangeKind::ConstAdd {
                    added: value.clone(),
                },
            }),
            (Some(l), Some(r)) if l != r => {
                if l.is_object() && r.is_object() {}
                (self.cb)(Change {
                    path: json_path.to_owned(),
                    change: ChangeKind::ConstRemove { removed: l.clone() },
                });
                (self.cb)(Change {
                    path: json_path.to_owned(),
                    change: ChangeKind::ConstAdd { added: r.clone() },
                });
            }
            _ => (),
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
            (self.cb)(Change {
                path: json_path.to_owned(),
                change: ChangeKind::PropertyRemove {
                    lhs_additional_properties,
                    removed: removed.clone(),
                },
            });
        }

        for added in rhs_props.difference(&lhs_props) {
            (self.cb)(Change {
                path: json_path.to_owned(),
                change: ChangeKind::PropertyAdd {
                    lhs_additional_properties,
                    added: added.clone(),
                },
            });
        }

        for common in rhs_props.intersection(&lhs_props) {
            let lhs_child = lhs.object().properties.get_mut(common.as_str()).unwrap();
            let rhs_child = rhs.object().properties.get_mut(common.as_str()).unwrap();

            let new_path = format!("{json_path}.{common}");
            self.diff(&new_path, lhs_child, rhs_child)?;
        }

        Ok(())
    }

    fn diff_additional_properties(
        &mut self,
        json_path: &str,
        lhs: &mut SchemaObject,
        rhs: &mut SchemaObject,
    ) -> Result<(), Error> {
        if let (Some(lhs_additional_properties), Some(rhs_additional_properties)) = (
            &mut lhs.object().additional_properties,
            &mut rhs.object().additional_properties,
        ) {
            if rhs_additional_properties != lhs_additional_properties {
                let new_path = format!("{json_path}.<additionalProperties>");

                self.diff(
                    &new_path,
                    lhs_additional_properties,
                    rhs_additional_properties,
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
        if let Some(diff) = diff(
            lhs.number_validation().minimum,
            rhs.number_validation().minimum,
            Range::Minimum,
        ) {
            (self.cb)(diff)
        }
        if let Some(diff) = diff(
            lhs.number_validation().maximum,
            rhs.number_validation().maximum,
            Range::Maximum,
        ) {
            (self.cb)(diff)
        }
        Ok(())
    }

    fn diff_array_items(
        &mut self,
        json_path: &str,
        lhs: &mut SchemaObject,
        rhs: &mut SchemaObject,
    ) -> Result<(), Error> {
        match (&mut lhs.array().items, &mut rhs.array().items) {
            (Some(SingleOrVec::Vec(lhs_items)), Some(SingleOrVec::Vec(rhs_items))) => {
                if lhs_items.len() != rhs_items.len() {
                    (self.cb)(Change {
                        path: json_path.to_owned(),
                        change: ChangeKind::TupleChange {
                            new_length: rhs_items.len(),
                        },
                    });
                }

                for (i, (lhs_inner, rhs_inner)) in
                    lhs_items.iter_mut().zip(rhs_items.iter_mut()).enumerate()
                {
                    let new_path = format!("{json_path}.{i}");
                    self.diff(&new_path, lhs_inner, rhs_inner)?;
                }
            }
            (Some(SingleOrVec::Single(lhs_inner)), Some(SingleOrVec::Single(rhs_inner))) => {
                let new_path = format!("{json_path}.?");
                self.diff(&new_path, lhs_inner, rhs_inner)?;
            }
            (Some(SingleOrVec::Single(lhs_inner)), Some(SingleOrVec::Vec(rhs_items))) => {
                (self.cb)(Change {
                    path: json_path.to_owned(),
                    change: ChangeKind::ArrayToTuple {
                        new_length: rhs_items.len(),
                    },
                });

                for (i, rhs_inner) in rhs_items.iter_mut().enumerate() {
                    let new_path = format!("{json_path}.{i}");
                    self.diff(&new_path, lhs_inner, rhs_inner)?;
                }
            }
            (Some(SingleOrVec::Vec(lhs_items)), Some(SingleOrVec::Single(rhs_inner))) => {
                (self.cb)(Change {
                    path: json_path.to_owned(),
                    change: ChangeKind::TupleToArray {
                        old_length: lhs_items.len(),
                    },
                });

                for (i, lhs_inner) in lhs_items.iter_mut().enumerate() {
                    let new_path = format!("{json_path}.{i}");
                    self.diff(&new_path, lhs_inner, rhs_inner)?;
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

    fn diff_required(
        &mut self,
        json_path: &str,
        lhs: &mut SchemaObject,
        rhs: &mut SchemaObject,
    ) -> Result<(), Error> {
        let lhs_required = &lhs.object().required;
        let rhs_required = &rhs.object().required;

        for removed in lhs_required.difference(rhs_required) {
            (self.cb)(Change {
                path: json_path.to_owned(),
                change: ChangeKind::RequiredRemove {
                    property: removed.clone(),
                },
            });
        }

        for added in rhs_required.difference(lhs_required) {
            (self.cb)(Change {
                path: json_path.to_owned(),
                change: ChangeKind::RequiredAdd {
                    property: added.clone(),
                },
            });
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

    fn restrictions_for_single_type(schema_object: &SchemaObject, ty: InstanceType) -> Schema {
        let mut ret = SchemaObject {
            instance_type: Some(SingleOrVec::Single(Box::new(ty))),
            ..Default::default()
        };
        match ty {
            InstanceType::String => ret.string = schema_object.string.clone(),
            InstanceType::Number | InstanceType::Integer => {
                ret.number = schema_object.number.clone()
            }
            InstanceType::Object => ret.object = schema_object.object.clone(),
            InstanceType::Array => ret.array = schema_object.array.clone(),
            _ => (),
        }
        Schema::Object(ret)
    }

    /// Split a schema into multiple schemas, one for each type in the multiple type.
    /// Returns the new schema and whether the schema was changed.
    fn split_types(schema_object: &mut SchemaObject) -> bool {
        let is_split = match schema_object.effective_type() {
            InternalJsonSchemaType::Multiple(types)
                if schema_object.subschemas().any_of.is_none() =>
            {
                *schema_object = SchemaObject {
                    subschemas: Some(Box::new(SubschemaValidation {
                        any_of: Some(
                            types
                                .into_iter()
                                .map(|ty| {
                                    Self::restrictions_for_single_type(schema_object, ty.into())
                                })
                                .collect(),
                        ),
                        ..Default::default()
                    })),
                    ..Default::default()
                };
                true
            }
            _ => false,
        };
        is_split
    }

    fn normalize_const(schema_object: &mut SchemaObject) {
        fn do_normalize(value: Value) -> SchemaObject {
            match value {
                Value::Object(obj) => {
                    let properties = obj
                        .into_iter()
                        .map(|(k, v)| (k, Schema::Object(do_normalize(v))))
                        .collect::<BTreeMap<_, _>>();
                    SchemaObject {
                        object: Some(Box::new(ObjectValidation {
                            properties,
                            ..Default::default()
                        })),
                        ..Default::default()
                    }
                }
                _ => SchemaObject {
                    const_value: Some(value),
                    ..Default::default()
                },
            }
        }
        if let Some(value) = schema_object.const_value.take() {
            *schema_object = do_normalize(value)
        }
    }

    fn do_diff(
        &mut self,
        json_path: &str,
        // Whether we are comparing elements in any_of subschemas
        comparing_any_of: bool,
        lhs: &mut SchemaObject,
        rhs: &mut SchemaObject,
    ) -> Result<(), Error> {
        self.resolve_references(lhs, rhs)?;
        let is_lhs_split = Self::split_types(lhs);
        let is_rhs_split = Self::split_types(rhs);
        self.diff_any_of(json_path, is_rhs_split, lhs, rhs)?;
        if !comparing_any_of {
            self.diff_instance_types(json_path, lhs, rhs);
        }
        self.diff_const(json_path, lhs, rhs);
        // If we split the types, we don't want to compare type-specific properties
        // because they are already compared in the `Self::diff_any_of`
        if !is_lhs_split && !is_rhs_split {
            self.diff_properties(json_path, lhs, rhs)?;
            self.diff_range(json_path, lhs, rhs)?;
            self.diff_additional_properties(json_path, lhs, rhs)?;
            self.diff_array_items(json_path, lhs, rhs)?;
            self.diff_required(json_path, lhs, rhs)?;
        }
        Ok(())
    }

    pub fn diff(
        &mut self,
        json_path: &str,
        lhs: &mut Schema,
        rhs: &mut Schema,
    ) -> Result<(), Error> {
        match (lhs, rhs) {
            (Schema::Object(lhs), Schema::Object(rhs)) => self.do_diff(json_path, false, lhs, rhs),
            (bool_lhs, Schema::Object(rhs)) => {
                self.do_diff(json_path, false, &mut bool_lhs.clone().into_object(), rhs)
            }
            (Schema::Object(lhs), bool_rhs) => {
                self.do_diff(json_path, false, lhs, &mut bool_rhs.clone().into_object())
            }
            (bool_lhs, bool_rhs) => self.do_diff(
                json_path,
                false,
                &mut bool_lhs.clone().into_object(),
                &mut bool_rhs.clone().into_object(),
            ),
        }
    }
}

trait JsonSchemaExt {
    fn is_true(&self) -> bool;
    fn effective_type(&mut self) -> InternalJsonSchemaType;
    /// Look for NumberValidation from "number" property in the schema.
    /// Check if `anyOf` subschema has NumberValidation, if the subschema is a single type.
    fn number_validation(&mut self) -> NumberValidation;
}

impl JsonSchemaExt for SchemaObject {
    fn is_true(&self) -> bool {
        *self == SchemaObject::default()
    }

    fn effective_type(&mut self) -> InternalJsonSchemaType {
        if let Some(ref ty) = self.instance_type {
            match ty {
                SingleOrVec::Single(ty) => JsonSchemaType::from(**ty).into(),
                SingleOrVec::Vec(tys) => InternalJsonSchemaType::Multiple(
                    tys.iter().copied().map(JsonSchemaType::from).collect(),
                ),
            }
        } else if let Some(ref constant) = self.const_value {
            serde_value_to_own(constant).into()
        } else if !self.object().properties.is_empty() {
            JsonSchemaType::Object.into()
        } else if let Some(ref any_of) = self.subschemas().any_of {
            InternalJsonSchemaType::Multiple(
                any_of
                    .iter()
                    .flat_map(|a| Self::effective_type(&mut a.clone().into_object()).explode())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
            )
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

    fn number_validation(&mut self) -> NumberValidation {
        let number_validation = self.number().clone();
        if number_validation == NumberValidation::default() {
            self.subschemas()
                .any_of
                .as_ref()
                .filter(|schemas| schemas.len() == 1)
                .and_then(|a| a.get(0))
                .map(|subschema| subschema.clone().into_object().number().clone())
                .unwrap_or_default()
        } else {
            number_validation
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
