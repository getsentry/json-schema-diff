use clap::Parser;
use std::fs::File;
use std::path::PathBuf;

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

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let lhs: serde_json::Value = serde_json::from_reader(File::open(args.lhs)?)?;
    let rhs: serde_json::Value = serde_json::from_reader(File::open(args.rhs)?)?;

    let changes = json_schema_diff::diff(lhs, rhs)?;

    for change in changes {
        println!("{}", serde_json::to_string(&change)?);
    }
    Ok(())
}
