use crate::cli::Args;
use clap::CommandFactory;
use clap_complete::{generate, Shell};

pub mod config;
pub mod connect;

pub fn print_completions(shell: Shell) {
    let cmd = &mut Args::command();
    generate(
        shell,
        cmd,
        cmd.get_name().to_string(),
        &mut std::io::stdout(),
    );
}
