use std::path::Path;

use anyhow::{Context, Result, bail};
use pce_core::{CreationDate, VisionName, create_vision};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let mut args = std::env::args().skip(1);
    let usage = "usage: pce vision new \"<name>\"";
    let (Some(verb), Some(action), Some(raw_name), None) =
        (args.next(), args.next(), args.next(), args.next())
    else {
        eprintln!("{usage}");
        bail!("{usage}");
    };

    if verb != "vision" || action != "new" {
        eprintln!("{usage}");
        bail!("{usage}");
    }

    let name = VisionName::parse(&raw_name).context("failed to parse vision name")?;
    let new_vision = create_vision(&name, Path::new("planning"), CreationDate::today())
        .context("failed to create vision")?;
    println!("{}", new_vision.dir());

    Ok(())
}
