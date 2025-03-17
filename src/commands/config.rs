use color_eyre::{eyre::eyre, Result};

use crate::{
    cli::{ListConfigSection, RemoveConfigSection, SetConfigSection},
    config::{Config, HostConfig, KeyAliasConfig},
};

pub fn list_config(config: &Config, command: ListConfigSection) -> Result<()> {
    match command {
        ListConfigSection::Alias => {
            let yaml = serde_yaml::to_string(&config.key_aliases)?;
            println!("{}", yaml);
        }
        ListConfigSection::Host => {
            let yaml = serde_yaml::to_string(&config.hosts)?;
            println!("{}", yaml);
        }
    }
    Ok(())
}

pub fn add_config(config: &mut Config, command: SetConfigSection) -> Result<()> {
    match command {
        SetConfigSection::Alias { kind } => {
            let name = kind.name();
            let alias_config: KeyAliasConfig = kind.into();
            config
                .key_aliases
                .entry(name.clone())
                .or_insert(alias_config);
            config.store()?;
            println!("Key alias '{name}' added");
        }
        SetConfigSection::Host {
            name,
            alias,
            args,
            destination,
        } => {
            // Ensure the key alias exists
            config
                .key_aliases
                .get(&alias)
                .ok_or_else(|| eyre!("Key alias '{alias}' not found"))?;

            let host = HostConfig {
                key_alias: alias,
                args,
                destination,
            };
            config.hosts.entry(name.clone()).or_insert(host);
            config.store()?;
            println!("Host '{name}' added");
        }
    }
    Ok(())
}

pub fn remove_config(config: &mut Config, command: RemoveConfigSection) -> Result<()> {
    match command {
        RemoveConfigSection::Alias { alias_name: alias } => {
            if !config.key_aliases.contains_key(&alias) {
                return Err(eyre!("Key alias '{alias}' not found"));
            }

            // Don't allow removing aliases that are used by any hosts
            let host_names: Vec<String> = config
                .hosts
                .iter()
                .filter_map(|(name, host)| {
                    if host.key_alias == alias {
                        Some(name.clone())
                    } else {
                        None
                    }
                })
                .collect();
            if !host_names.is_empty() {
                return Err(eyre!(
                    "Key alias '{alias}' cannot be removed because it is used by the following hosts: {host_names:?}"
                ));
            }

            config.key_aliases.remove(&alias);
            config.store()?;
            println!("Key alias '{alias}' removed");
        }
        RemoveConfigSection::Host { name } => {
            if !config.hosts.contains_key(&name) {
                return Err(eyre!("Host '{name}' not found"));
            }
            config.hosts.remove(&name);
            config.store()?;
            println!("Host '{name}' removed");
        }
    }
    Ok(())
}
