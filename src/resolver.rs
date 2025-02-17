use std::collections::BTreeMap;

use schemars::schema::{RootSchema, Schema, SchemaObject};

pub struct Resolver {
    ref_lookup: BTreeMap<String, String>,
}

impl Resolver {
    pub fn for_schema(root: &RootSchema) -> Self {
        let mut ref_lookup = BTreeMap::new();

        for (key, schema) in &root.definitions {
            if let Some(id) = schema.get_schema_id() {
                ref_lookup.insert(id.to_owned(), key.clone());
            }

            if let Some(root_id) = root.schema.get_schema_id() {
                ref_lookup.insert(format!("{root_id}#/definitions/{key}"), key.clone());
                ref_lookup.insert(format!("{root_id}#/$defs/{key}"), key.clone());
            }

            ref_lookup.insert(format!("#/definitions/{key}"), key.clone());
            ref_lookup.insert(format!("#/$defs/{key}"), key.clone());
        }

        Self { ref_lookup }
    }

    /// Resolves a reference.
    ///
    /// `root` must be the same schema that was used to construct the resolver.
    /// This is not checked.
    pub fn resolve<'a>(&self, root: &'a RootSchema, reference: &str) -> Option<&'a Schema> {
        let key = self.ref_lookup.get(reference)?;
        root.definitions.get(key)
    }
}

trait MayHaveSchemaId {
    fn get_schema_id(&self) -> Option<&str>;
}

impl MayHaveSchemaId for SchemaObject {
    fn get_schema_id(&self) -> Option<&str> {
        self.metadata
            .as_ref()
            .and_then(|m| m.id.as_ref())
            .map(|id| id.as_str())
    }
}

impl MayHaveSchemaId for Schema {
    fn get_schema_id(&self) -> Option<&str> {
        match self {
            Schema::Object(schema_obj) => schema_obj.get_schema_id(),
            Schema::Bool(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft7_definitions() {
        let root: RootSchema = serde_json::from_str(
            r#"{
                "definitions": {
                    "A": {}
                }
            }"#,
        )
        .unwrap();
        let resolver = Resolver::for_schema(&root);

        let resolved = resolver.resolve(&root, "#/definitions/A");
        assert!(resolved.is_some());

        let resolved = resolver.resolve(&root, "#/definitions/not-there");
        assert!(resolved.is_none());
    }

    #[test]
    fn draft7_root_has_id() {
        let root: RootSchema = serde_json::from_str(
            r#"{
                "$id": "urn:uuid:e773a2e8-d746-4dc6-9480-0bba5ff33504",
                "definitions": {
                    "A": {}
                }
            }"#,
        )
        .unwrap();
        let resolver = Resolver::for_schema(&root);

        let resolved = resolver.resolve(&root, "#/definitions/A");
        assert!(resolved.is_some());
        let resolved = resolver.resolve(
            &root,
            "urn:uuid:e773a2e8-d746-4dc6-9480-0bba5ff33504#/definitions/A",
        );
        assert!(resolved.is_some());
    }

    #[test]
    fn draft7_definition_has_id() {
        let root: RootSchema = serde_json::from_str(
            r#"{
                "definitions": {
                    "A": {
                        "$id": "some-id"
                    }
                }
            }"#,
        )
        .unwrap();
        let resolver = Resolver::for_schema(&root);

        let resolved = resolver.resolve(&root, "some-id");
        assert!(resolved.is_some());
        assert_eq!(resolved, resolver.resolve(&root, "#/definitions/A"))
    }

    #[test]
    fn draft2020_12_defs() {
        let root: RootSchema = serde_json::from_str(
            r#"{
                "$defs": {
                    "A": {
                        "$id": "some-id"
                    }
                }
            }"#,
        )
        .unwrap();
        let resolver = Resolver::for_schema(&root);

        let resolved = resolver.resolve(&root, "#/$defs/A");
        assert!(resolved.is_some());
        assert_eq!(resolved, resolver.resolve(&root, "some-id"));

        let resolved = resolver.resolve(&root, "#/$defs/not-there");
        assert!(resolved.is_none());
    }
}
