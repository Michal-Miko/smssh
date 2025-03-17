use color_eyre::{
    eyre::{bail, eyre, Context},
    Result,
};
use nix::{
    libc::{tcsetpgrp, SIGTTOU},
    unistd::{getpgid, setpgid, Pid},
};
use std::io::Write;
use std::{
    io,
    process::{Command, ExitCode, Stdio},
    sync::{atomic::AtomicBool, Arc},
};

use std::{fs::Permissions, os::unix::fs::PermissionsExt};
use tempfile::{NamedTempFile, TempDir};

use crate::config::{Config, KeyAliasConfig};

fn create_key_directory() -> Result<TempDir> {
    let dir = tempfile::Builder::new()
        .permissions(Permissions::from_mode(0o700))
        .tempdir_in("/dev/shm")
        .or_else(|_| {
            tempfile::Builder::new()
                .permissions(Permissions::from_mode(0o700))
                .tempdir()
        })?;
    Ok(dir)
}

fn create_key_file(dir: &TempDir) -> Result<NamedTempFile> {
    let file = tempfile::Builder::new()
        .permissions(Permissions::from_mode(0o600))
        .tempfile_in(dir)?;
    std::fs::set_permissions(file.path(), Permissions::from_mode(0o400))?;
    Ok(file)
}

fn pull_key(alias: &KeyAliasConfig, key_file: &mut NamedTempFile) -> Result<()> {
    let key = match alias {
        KeyAliasConfig::SecretsManager { secret_arn } => crate::aws::get_key_blocking(secret_arn)?,
    };
    key_file.write_all(key.as_bytes())?;
    Ok(())
}

pub fn connect_by_alias(key_alias: &str, config: &Config, ssh_args: &[String]) -> Result<()> {
    let key_alias_config = config
        .key_aliases
        .get(key_alias)
        .ok_or(eyre!("Key alias '{key_alias}' does not exist"))?;

    connect(key_alias_config, None, ssh_args)
}

pub fn connect_by_host(host_config: &str, config: &Config, ssh_args: &[String]) -> Result<()> {
    let host_config = config
        .hosts
        .get(host_config)
        .ok_or(eyre!("Host '{host_config}' does not exist"))?;

    let key_alias_config = config.key_aliases.get(&host_config.key_alias).ok_or(eyre!(
        "Key alias '{}' configured in '{host_config}' does not exist",
        host_config.key_alias
    ))?;

    connect(key_alias_config, Some(&host_config.destination), ssh_args)
}

pub fn connect(
    key_alias_config: &KeyAliasConfig,
    destination: Option<&str>,
    ssh_args: &[String],
) -> Result<()> {
    let key_dir = create_key_directory()?;
    let mut key_file = create_key_file(&key_dir)?;

    pull_key(key_alias_config, &mut key_file)?;

    let mut command = Command::new("ssh");
    command.arg("-i");
    command.arg(key_file.path());
    command.args(ssh_args);

    if let Some(destination) = destination {
        command.arg(destination);
    }

    println!("Running ssh command: {:?}", command);

    run_command_in_foreground(command)?;
    Ok(())
}

fn run_command_in_foreground(mut command: Command) -> Result<ExitCode> {
    let mut child = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    // Change PGID to isolate signal handling
    let child_pid = Pid::from_raw(child.id() as i32);
    match setpgid(child_pid, child_pid) {
        Ok(_) => {}
        Err(nix::errno::Errno::ESRCH) => {
            bail!("Failed to set child PGID")
        }
        Err(e) => Err(io::Error::from_raw_os_error(e as i32)).context("Set child PGID")?,
    }

    // Set the foreground PGID to the child PID
    // TODO: check if this is still needed
    let stop_on_sigttou = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register_conditional_default(SIGTTOU, stop_on_sigttou.clone())?;

    let fgpgid_result = unsafe { tcsetpgrp(nix::libc::STDIN_FILENO, child_pid.as_raw()) };

    stop_on_sigttou.store(true, std::sync::atomic::Ordering::Relaxed);

    if fgpgid_result != 0 {
        Err(io::Error::from_raw_os_error(fgpgid_result))?
    }

    let status = child.wait()?;

    // Restore foreground PGID
    let pgid = getpgid(None)?;
    let fpgid_result = unsafe { tcsetpgrp(nix::libc::STDIN_FILENO, pgid.as_raw()) };
    if fpgid_result != 0 {
        Err(io::Error::from_raw_os_error(fpgid_result))?
    }

    let exit_code = status
        .code()
        .ok_or(eyre!("Child exited without status code"))?;

    Ok(ExitCode::from(exit_code as u8))
}
