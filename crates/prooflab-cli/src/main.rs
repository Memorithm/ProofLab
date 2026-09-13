//! `prooflab` executable entry point.

#![forbid(unsafe_code)]

use clap::Parser;
use prooflab_cli::{Cli, run};

fn main() -> std::process::ExitCode {
    run(Cli::parse())
}
