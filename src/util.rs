use anyhow::{Context, Result};
use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

/// Invokes `cargo` with the subcommand `command` and the supplied `args`.
///
/// In case `working_dir` is set, the command will be invoked with that folder
/// as the working directory.
///
/// In case `env` is given environment variables can be either set or unset:
///   * To _set_ push an item a la `("VAR_NAME", Some("VAR_VALUE"))` to
///     the `env` vector.
///   * To _unset_ push an item a la `("VAR_NAME", None)` to the `env`
///     vector.
///
/// If successful, returns the stdout bytes.
pub(crate) fn invoke_cargo<I, S, P>(
    command: &str,
    args: I,
    working_dir: Option<P>,
    env: Vec<(&str, Option<&str>)>,
) -> Result<Vec<u8>>
where
    I: IntoIterator<Item = S> + std::fmt::Debug,
    S: AsRef<OsStr>,
    P: AsRef<Path>,
{
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut cmd = Command::new(cargo);

    env.iter().for_each(|(env_key, maybe_env_val)| {
        match maybe_env_val {
            Some(env_val) => cmd.env(env_key, env_val),
            None => cmd.env_remove(env_key),
        };
    });

    if let Some(path) = working_dir {
        log::debug!("Setting cargo working dir to '{}'", path.as_ref().display());
        cmd.current_dir(path);
    }

    cmd.arg(command);
    cmd.args(args);

    log::info!("Invoking cargo: {:?}", cmd);

    let child = cmd
        // capture the stdout to return from this function as bytes
        .stdout(std::process::Stdio::piped())
        .spawn()
        .context(format!("Error executing `{:?}`", cmd))?;
    let output = child.wait_with_output()?;

    if output.status.success() {
        Ok(output.stdout)
    } else {
        anyhow::bail!("`{:?}` failed with exit code: {:?}", cmd, output.status.code());
    }
}
