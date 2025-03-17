use color_eyre::{Result, eyre::eyre};
use crossterm::ExecutableCommand;
use crossterm::cursor;
use nix::sys::signal;
use nix::{
    libc::{STDIN_FILENO, tcsetpgrp},
    sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction},
    unistd::{Pid, getpid, setpgid},
};
use signal_hook::consts::signal::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
use std::io::stdout;
use std::{
    io,
    process::{Command, Stdio},
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
    println!("Fetching the key");
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

fn register_termination_handlers(term_flag: Arc<AtomicBool>) -> Result<()> {
    signal_hook::flag::register(SIGHUP, term_flag.clone())?;
    signal_hook::flag::register(SIGINT, term_flag.clone())?;
    signal_hook::flag::register(SIGTERM, term_flag.clone())?;
    signal_hook::flag::register(SIGQUIT, term_flag)?;
    Ok(())
}

pub fn connect(
    key_alias_config: &KeyAliasConfig,
    destination: Option<&str>,
    ssh_args: &[String],
) -> Result<()> {
    let key_dir = create_key_directory()?;
    let mut key_file = create_key_file(&key_dir)?;
    let term_flag = Arc::new(AtomicBool::new(false));
    register_termination_handlers(term_flag.clone())?;

    pull_key(key_alias_config, &mut key_file)?;

    let mut command = Command::new("ssh");
    command.arg("-i");
    command.arg(key_file.path());
    command.args(ssh_args);

    if let Some(destination) = destination {
        command.arg(destination);
    }

    println!("Running ssh command: {:?}", command);
    run_command_in_foreground(command, term_flag)
}

fn run_command_in_foreground(mut command: Command, term_flag: Arc<AtomicBool>) -> Result<()> {
    let mut child = unsafe {
        command
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .pre_exec(|| {
                // Detach from the parent PGID
                let pid = getpid();
                setpgid(pid, pid)?;
                Ok(())
            })
            .spawn()?
    };

    // Ignore SIGTTOU to allow moving the parent to the foreground after the child exits
    // and to allow background logging if `tostop` is set. A custom handler would not work,
    // the signal needs to be ignored or blocked for the background tcsetpgrp call to succeed
    let ignore_action = SigAction::new(SigHandler::SigIgn, SaFlags::empty(), SigSet::empty());
    let old_action = unsafe { sigaction(Signal::SIGTTOU, &ignore_action)? };

    // Set the foreground PGID to the child's PGID
    let child_pid = Pid::from_raw(child.id() as i32);
    let fgpgid_result = unsafe { tcsetpgrp(STDIN_FILENO, child_pid.as_raw()) };
    if fgpgid_result != 0 {
        Err(io::Error::last_os_error())?
    }

    // Wait for the child to exit
    loop {
        // Termination requested
        if term_flag.load(std::sync::atomic::Ordering::Relaxed) {
            let mut stdout = stdout();
            stdout.flush()?;
            stdout.execute(cursor::MoveToNextLine(1))?;
            println!("\nTermination signal received, exiting...");

            signal::kill(child_pid, Signal::SIGTERM).or_else(|_| child.kill())?;
            child.wait()?;

            stdout.flush()?;
            stdout.execute(cursor::MoveToNextLine(1))?;
            break;
        }

        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            Err(e) => {
                println!("Error waiting for child: {:?}", e);
                break;
            }
        }
    }

    // Set the foreground PGID to the parent's PGID
    // The parent process is in the background - this requires ignoring or blocking SIGTTOU
    let parent_pid = getpid();
    let fgpgid_result = unsafe { tcsetpgrp(STDIN_FILENO, parent_pid.as_raw()) };
    if fgpgid_result != 0 {
        Err(io::Error::last_os_error())?
    }

    // Restore the SIGTTOU handler now that we're in the foreground again
    unsafe { sigaction(Signal::SIGTTOU, &old_action)? };

    Ok(())
}
