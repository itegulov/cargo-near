use crate::crate_metadata::CrateMetadata;
use anyhow::{anyhow, Context, Result};
use cargo_metadata::Message;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
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
        anyhow::bail!(
            "`{:?}` failed with exit code: {:?}",
            cmd,
            output.status.code()
        );
    }
}

fn build_cargo_project(crate_metadata: &CrateMetadata) -> anyhow::Result<Vec<Message>> {
    let target_dir_arg = format!(
        "--target-dir={}",
        crate_metadata.target_directory.to_string_lossy()
    );
    let output = invoke_cargo(
        "build",
        &["--release", "--message-format=json", &target_dir_arg],
        crate_metadata.manifest_path.directory(),
        vec![],
    )?;

    let reader = std::io::BufReader::new(output.as_slice());
    Ok(Message::parse_stream(reader).map(|m| m.unwrap()).collect())
}

/// Builds the cargo project located at `project_path` and returns the path to generated .so file contents.
pub(crate) fn compile_project(crate_metadata: &CrateMetadata) -> anyhow::Result<PathBuf> {
    let messages = build_cargo_project(crate_metadata)?;
    // We find the last compiler artifact message which should contain information about the
    // resulting .so file
    let compile_artifact = messages
        .iter()
        .filter_map(|m| match m {
            cargo_metadata::Message::CompilerArtifact(artifact) => Some(artifact),
            _ => None,
        })
        .last()
        .ok_or(anyhow!(
            "Cargo failed to produce any compilation artifacts. \
                 Please check that your project contains a NEAR smart contract."
        ))?;
    // The project could have generated many auxiliary files, we are only interested in .so files
    let so_files = compile_artifact
        .filenames
        .to_owned()
        .into_iter()
        .filter(|f| f.as_str().ends_with(".so"))
        .collect::<Vec<_>>();
    match so_files.as_slice() {
        [] => Err(anyhow!(
            "Compilation resulted in no '.so' target files. \
                 Please check that your project contains a NEAR smart contract."
        )),
        [file] => Ok(file.to_owned().into_std_path_buf()),
        _ => Err(anyhow!(
            "Compilation resulted in more than one '.so' target file: {:?}",
            so_files
        )),
    }
}
