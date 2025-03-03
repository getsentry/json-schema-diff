#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

use schemars::schema::{RootSchema, Schema};
use serde_json::Value;
use thiserror::Error;

mod diff_walker;
mod resolver;
mod types;

pub use types::*;

/// Take two JSON schemas, and compare them.
///
/// `lhs` (left-hand side) is the old schema, `rhs` (right-hand side) is the new schema.
pub fn diff(lhs: Value, rhs: Value) -> Result<Vec<Change>, Error> {
    let lhs_root: RootSchema = serde_json::from_value(lhs)?;
    let rhs_root: RootSchema = serde_json::from_value(rhs)?;

    let mut changes = vec![];
    let mut walker = diff_walker::DiffWalker::new(
        |change: Change| {
            changes.push(change);
        },
        lhs_root,
        rhs_root,
    );
    walker.diff(
        "",
        &mut Schema::Object(walker.lhs_root.schema.clone()),
        &mut Schema::Object(walker.rhs_root.schema.clone()),
    )?;
    Ok(changes)
}
