use std::fmt::{Display, Formatter};

use clap::{Parser, Subcommand};
use clap_complete::Shell;
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// The subcommand to run
    #[command(subcommand)]
    pub command: SMSSHCommand,
}

#[derive(Subcommand, Debug)]
#[command()]
pub enum SMSSHCommand {
    /// Connect to a remote machine using the host configuration
    #[command(alias = "c")]
    Connect {
        /// The host configuration to use
        #[arg()]
        host: String,
        /// The arguments to pass to the SSH command
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        ssh_args: Vec<String>,
    },
    /// Connect to a remote machine using the specified key alias
    #[command(alias = "ca")]
    ConnectWithAlias {
        /// The key alias to use
        #[arg()]
        key_alias: String,
        /// The arguments to pass to the SSH command
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        ssh_args: Vec<String>,
    },
    /// Manage the SSH configuration
    #[command(alias = "cfg")]
    Config {
        #[command(subcommand)]
        command: SSHConfig,
    },
    /// Generate shell completions
    #[command()]
    Completions {
        /// The shell to generate completions for
        #[arg(short, long, value_enum, default_value_t = Shell::Fish)]
        shell: Shell,
    },
}

#[derive(Subcommand, Debug)]
pub enum SSHConfig {
    /// List the configured key aliases
    #[command(alias = "l")]
    List {
        /// The SSH configuration section to list
        #[command(subcommand)]
        section: ListConfigSection,
    },
    /// Add a new configuration entry
    #[command(alias = "s")]
    Set {
        /// The SSH configuration section to modify
        #[command(subcommand)]
        section: SetConfigSection,
    },
    /// Remove a configuration entry
    #[command(alias = "r")]
    Remove {
        /// The key alias to remove
        #[command(subcommand)]
        section: RemoveConfigSection,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ListConfigSection {
    /// Manage the key aliases
    #[command(alias = "a")]
    Alias,
    /// Manage the SSH hosts
    #[command(alias = "h")]
    Host,
}

#[derive(Subcommand, Debug)]
pub enum SetConfigSection {
    /// Add a new key alias
    #[command(alias = "a")]
    Alias {
        /// Alias kind
        #[command(subcommand)]
        kind: AliasKind,
    },
    /// Add a new host configuration
    #[command(alias = "h")]
    Host {
        /// Name of this host configuration
        #[arg(short = 'n', long)]
        name: String,
        /// Name of an existing key alias to use as the SSH private key
        #[arg(short = 'a', long)]
        alias: String,
        /// SSH destination, example: user@hostname
        #[arg(short = 'd', long)]
        destination: String,
        /// Extra SSH arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum RemoveConfigSection {
    /// Remove a key alias
    #[command(alias = "a")]
    Alias {
        /// The key alias to remove
        #[arg()]
        alias_name: String,
    },
    /// Remove a host configuration
    #[command(alias = "h")]
    Host {
        /// Name of the host configuration to remove
        #[arg()]
        name: String,
    },
}

#[derive(Subcommand, Serialize, Deserialize, Debug)]
pub enum AliasKind {
    /// Secrets Manager secret containing the SSH private key
    #[command(alias = "sm")]
    SecretsManager {
        /// Alias name
        #[arg(short = 'n', long)]
        name: String,
        /// ARN of the Secrets Manager secret containing the SSH private key
        #[arg(short = 'a', long)]
        secret_arn: String,
    },
}

impl AliasKind {
    pub fn name(&self) -> String {
        match self {
            AliasKind::SecretsManager { name, .. } => name.clone(),
        }
    }
}

impl Display for AliasKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let yaml = serde_yaml::to_string(self).map_err(|_| std::fmt::Error)?;
        write!(f, "{}", yaml)
    }
}
