mod app;
mod geometry;
mod render;
mod style;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();

    if cli.version {
        println!("scrop {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    match cli.command {
        Some(Command::Select) | None => match app::run(cli.verbose) {
            Ok(Some(selection)) => {
                println!("{}", selection.to_slurp_geometry());
                ExitCode::SUCCESS
            }
            Ok(None) => ExitCode::from(1),
            Err(error) => {
                eprintln!("scrop: {error}");
                ExitCode::from(2)
            }
        },
    }
}

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Precise Wayland region selector",
    disable_version_flag = true
)]
struct Cli {
    /// Print version
    #[arg(short = 'v', long = "version", global = true)]
    version: bool,

    /// Increase diagnostic output; may be repeated
    #[arg(long = "verbose", action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Interactive region selection
    Select,
}
