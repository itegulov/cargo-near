use platforms::{TARGET_ARCH, TARGET_ENV, TARGET_OS};
use std::borrow::Cow;
use std::process::Command;

fn main() {
    generate_cargo_keys();

    std::process::exit(0);
}

/// Generate the `cargo:` key output
fn generate_cargo_keys() {
    let output = Command::new("git").args(&["rev-parse", "--short", "HEAD"]).output();

    let commit = match output {
        Ok(o) if o.status.success() => {
            let sha = String::from_utf8_lossy(&o.stdout).trim().to_owned();
            Cow::from(sha)
        }
        Ok(o) => {
            println!("cargo:warning=Git command failed with status: {}", o.status);
            Cow::from("unknown")
        }
        Err(err) => {
            println!("cargo:warning=Failed to execute git command: {}", err);
            Cow::from("unknown")
        }
    };

    println!("cargo:rustc-env=CARGO_NEAR_CLI_IMPL_VERSION={}", get_version(&commit))
}

fn get_version(impl_commit: &str) -> String {
    let commit_dash = if impl_commit.is_empty() { "" } else { "-" };

    format!(
        "{}{}{}-{}",
        std::env::var("CARGO_PKG_VERSION").unwrap_or_default(),
        commit_dash,
        impl_commit,
        get_platform(),
    )
}

fn get_platform() -> String {
    let env_dash = if TARGET_ENV.is_some() { "-" } else { "" };

    format!(
        "{}-{}{}{}",
        TARGET_ARCH.as_str(),
        TARGET_OS.as_str(),
        env_dash,
        TARGET_ENV.map(|x| x.as_str()).unwrap_or(""),
    )
}
