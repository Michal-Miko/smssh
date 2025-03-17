use color_eyre::{Result, eyre::eyre};
use nix::{
    libc::{SIGTTOU, STDIN_FILENO, tcgetpgrp, tcsetpgrp},
    unistd::{Pid, getpgid, getpid, setpgid},
};
use std::{
    io,
    process::{Command, ExitCode, Stdio},
    sync::{Arc, atomic::AtomicBool},
};
use std::{io::Write, os::unix::process::CommandExt};

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

    // let mut command = Command::new("ssh");
    // command.arg("-i");
    // command.arg(key_file.path());
    // command.args(ssh_args);
    //
    // if let Some(destination) = destination {
    //     command.arg(destination);
    // }

    let mut command = Command::new("sleep");
    command.arg("300");

    println!("Key file: {:?}", key_file.path());
    println!("Running ssh command: {:?}", command);

    run_command_in_foreground(command)?;
    Ok(())
}

fn run_command_in_foreground(mut command: Command) -> Result<ExitCode> {
    let mut child = unsafe {
        command
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .pre_exec(|| {
                let pid = getpid();
                setpgid(pid, pid)?;
                Ok(())
            })
            .spawn()?
    };

    // Ignore SIGTTOU while the parent is in the background
    let stop_on_sigttou = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register_conditional_default(SIGTTOU, stop_on_sigttou.clone())?;

    // Set the foreground PGID to the child PID
    let child_pid = Pid::from_raw(child.id() as i32);
    let fgpgid_result = unsafe { tcsetpgrp(STDIN_FILENO, child_pid.as_raw()) };

    if fgpgid_result != 0 {
        Err(io::Error::from_raw_os_error(fgpgid_result))?
    }

    let status = child.wait()?;

    // Restore foreground PGID
    let pgid = getpgid(None)?;
    let fg_pgrp = unsafe { tcgetpgrp(STDIN_FILENO) };
    println!("FG PGRP: {fg_pgrp}");
    println!("Parent PGRP: {}", pgid.as_raw());
    println!("Child PGRP: {}", child_pid.as_raw());
    let fpgid_result = unsafe { tcsetpgrp(STDIN_FILENO, pgid.as_raw()) };
    if fpgid_result != 0 {
        Err(io::Error::from_raw_os_error(fpgid_result))?
    }

    stop_on_sigttou.store(true, std::sync::atomic::Ordering::Relaxed);

    let exit_code = status
        .code()
        .ok_or(eyre!("Child exited without status code"))?;
    println!("Exited");

    Ok(ExitCode::from(exit_code as u8))
}
