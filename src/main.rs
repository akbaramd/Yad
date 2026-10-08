use anyhow::Result;
use clap::Parser;
use yad::{app, cli::Cli};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {:#}", err);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    app::execute(cli)
}
