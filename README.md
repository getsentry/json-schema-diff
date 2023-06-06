# json-schema-diff

A work-in-progress tool to diff changes between JSON schemas. A lot of JSON
schema features are not implemented and therefore ignored, see [the issue
tracker](https://github.com/getsentry/json-schema-diff/issues).

Use this tool as a best-effort to find obviously breaking changes in CI, but not for much more.

This crate is used with draft-07 but even that is work in progress.

## Usage via CLI

[Install Rust](https://rustup.rs/) and:

```bash
cargo install json-schema-diff

cat schema-old.json schema-new.json
# {"type": "string"}
# {"type": "boolean"}

cargo run --features=build-binary -- \
    schema-old.json \
    schema-new.json
# {"path":"","change":{"TypeRemove":{"removed":"string"}},"is_breaking":true}
# {"path":"","change":{"TypeAdd":{"added":"boolean"}},"is_breaking":false}
```

Sentry uses this tool in
[`sentry-kafka-schemas`](https://github.com/getsentry/sentry-kafka-schemas) to
annotate pull requests with breaking changes made to schema definitions. It
invokes the CLI tool on the schema from master vs the schema in the PR, and
post-processes the output using a Python script for human consumption.

`is_breaking` is just a suggestion. You may choose to ignore it entirely and
instead define which kinds of changes are breaking to you in wrapper scripts.

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
            change: ChangeKind::TypeRemove { removed: JsonSchemaType::String }
        },
        Change {
            path: "".to_owned(),
            change: ChangeKind::TypeAdd { added: JsonSchemaType::Boolean }
        }
    ]
);
```

## License

Licensed under Apache 2.0
