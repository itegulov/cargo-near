use anyhow::{Context, Result};
use cargo_metadata::{Metadata as CargoMetadata, MetadataCommand, Package};
use std::path::PathBuf;

use crate::workspace::ManifestPath;

/// Relevant metadata obtained from Cargo.toml.
#[derive(Debug)]
pub struct CrateMetadata {
    pub manifest_path: ManifestPath,
    pub cargo_meta: cargo_metadata::Metadata,
    pub root_package: Package,
    pub target_directory: PathBuf,
}

impl CrateMetadata {
    /// Parses the contract manifest and returns relevant metadata.
    pub fn collect(manifest_path: &ManifestPath) -> Result<Self> {
        let (metadata, root_package) = get_cargo_metadata(manifest_path)?;
        let mut target_directory = metadata.target_directory.as_path().join("near");

        // Normalize the package and lib name.
        let package_name = root_package.name.replace('-', "_");

        let absolute_manifest_path = manifest_path.absolute_directory()?;
        let absolute_workspace_root = metadata.workspace_root.canonicalize()?;
        if absolute_manifest_path != absolute_workspace_root {
            // If the contract is a package in a workspace, we use the package name
            // as the name of the sub-folder where we put the `.contract` bundle.
            target_directory = target_directory.join(package_name);
        }

        let crate_metadata = CrateMetadata {
            manifest_path: manifest_path.clone(),
            cargo_meta: metadata,
            root_package,
            target_directory: target_directory.into(),
        };
        Ok(crate_metadata)
    }
}

/// Get the result of `cargo metadata`, together with the root package id.
fn get_cargo_metadata(manifest_path: &ManifestPath) -> Result<(CargoMetadata, Package)> {
    log::info!(
        "Fetching cargo metadata for {}",
        manifest_path.as_ref().to_string_lossy()
    );
    let mut cmd = MetadataCommand::new();
    let metadata = cmd
        .manifest_path(manifest_path.as_ref())
        .exec()
        .context("Error invoking `cargo metadata`")?;
    let root_package_id = metadata
        .resolve
        .as_ref()
        .and_then(|resolve| resolve.root.as_ref())
        .context("Cannot infer the root project id")?
        .clone();
    // Find the root package by id in the list of packages. It is logical error if the root
    // package is not found in the list.
    let root_package = metadata
        .packages
        .iter()
        .find(|package| package.id == root_package_id)
        .expect("The package is not found in the `cargo metadata` output")
        .clone();
    Ok((metadata, root_package))
}
