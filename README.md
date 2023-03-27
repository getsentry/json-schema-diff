# json-schema-diff

A work-in-progress tool to diff changes between code schemas. A lot of JSON schema features are not implemented and therefore ignored, such as:

* `required`
* `patternProperties` (entirely ignored)
* `const` (changes from `{"const": "foo"}` to `{"type": "string"}` are not detected)

Use this tool as a best-effort to find obviously breaking changes in CI, but not for much more.

This crate is used with draft-07 but even that is work in progress.

## Usage via CLI

```bash
cargo run --features=build-binary -- \
    schema-old.json \
    schema-new.json
```

## Usage as library

```rust
use json_schema_diff::*;

let lhs = serde_json::json! {{ 
    "type": "string",
}};
let rhs = serde_json::json! {{ 
    "type": "boolean",
}};

assert_eq!(
    json_schema_diff::diff(lhs, rhs).unwrap(),
    vec![
        Change {
            path: "".to_owned(),
            change: ChangeKind::TypeRemove { removed: SimpleJsonSchemaType::String }
        },
        Change {
            path: "".to_owned(),
            change: ChangeKind::TypeAdd { added: SimpleJsonSchemaType::Boolean }
        }
    ]
);
```

## License

Licensed under Apache 2.0, see `./LICENSE`
