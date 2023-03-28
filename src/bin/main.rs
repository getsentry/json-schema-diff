use std::fs::File;
use std::path::PathBuf;

use clap::Parser;
use serde::Serialize;
use anyhow::Error;

/// Compare old and new schema, and print differences
#[derive(Parser)]
#[clap(about, version)]
struct Args {
    /// The old schema
    lhs: PathBuf,
    /// The new schema
    rhs: PathBuf,
}

#[derive(Serialize)]
struct Change {
    #[serde(flatten)]
    inner: json_schema_diff::Change,
    is_breaking: bool

}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let lhs: serde_json::Value = serde_json::from_reader(File::open(args.lhs)?)?;
    let rhs: serde_json::Value = serde_json::from_reader(File::open(args.rhs)?)?;

    let changes = json_schema_diff::diff(lhs, rhs)?;

    for change in changes {
        let is_breaking = change.change.is_breaking();
        let change = Change { inner: change, is_breaking };
        println!("{}", serde_json::to_string(&change)?);
    }
    Ok(())
}
