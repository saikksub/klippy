use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use crux_core::type_generation::facet::{Config, TypeRegistry};

use shared::Klippy;

/// Generates Swift types for the `Event`, `ViewModel` and `Effect` types.
#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    output_dir: PathBuf,
}

fn main() -> Result<()> {
    pretty_env_logger::init();
    let args = Args::parse();

    let typegen_app = TypeRegistry::new().register_app::<Klippy>()?.build()?;
    let config = Config::builder("App", &args.output_dir).build();
    typegen_app.swift(&config)?;
    log::info!("Swift types written to {}", args.output_dir.display());

    Ok(())
}
