//! sigma-updates-cli — list, check, and publish Debian packages.

#![forbid(unsafe_code)]

mod cli;
mod command;

use std::process::ExitCode;

use clap::Parser;
use sigma_updates_client::UpdatesClient;

use crate::cli::Cli;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    let mut client = UpdatesClient::new(cli.base_url());
    if let Some(token) = cli.auth_token()? {
        client = client.with_token(token);
    }
    cli.command.run(&client)
}
