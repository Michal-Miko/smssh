use clap::Parser;
use cli::{Args, SMSSHCommand, SSHConfig};
use color_eyre::Result;

mod aws;
mod cli;
mod commands;
mod config;

fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    let mut config = config::Config::load()?;

    match args.command {
        SMSSHCommand::Connect { host, ssh_args } => {
            commands::connect::connect_by_host(&host, &config, &ssh_args)?
        }

        SMSSHCommand::ConnectWithAlias {
            key_alias,
            ssh_args,
        } => commands::connect::connect_by_alias(&key_alias, &config, &ssh_args)?,

        SMSSHCommand::Config { command } => match command {
            SSHConfig::List { section } => commands::config::list_config(&config, section)?,
            SSHConfig::Set { section } => commands::config::add_config(&mut config, section)?,
            SSHConfig::Remove { section } => commands::config::remove_config(&mut config, section)?,
        },

        SMSSHCommand::Completions { shell } => commands::print_completions(shell),
    }

    Ok(())
}
