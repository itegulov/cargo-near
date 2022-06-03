use crate::crate_metadata::CrateMetadata;
use crate::util;
use crate::workspace::{ManifestPath, Workspace};
use anyhow::Result;
use near_sdk::__private::{AbiMetainfo, AbiRoot};
use std::collections::HashMap;
use std::{fs, path::PathBuf};

const METADATA_FILE: &str = "abi.json";

/// Metadata generation result.
#[derive(serde::Serialize)]
pub struct MetadataResult {
    /// Path to the resulting metadata file.
    pub dest_metadata: PathBuf,
}

fn extract_metainfo(crate_metadata: &CrateMetadata) -> AbiMetainfo {
    let package = &crate_metadata.root_package;
    AbiMetainfo {
        name: Some(package.name.clone()),
        version: Some(package.version.to_string()),
        authors: package.authors.clone(),
        other: HashMap::new(),
    }
}

pub(crate) fn execute(crate_metadata: &CrateMetadata) -> Result<MetadataResult> {
    let target_directory = crate_metadata.target_directory.clone();
    let out_path_metadata = target_directory.join(METADATA_FILE);

    let generate_metadata = |manifest_path: &ManifestPath| -> Result<()> {
        let target_dir_arg = format!("--target-dir={}", target_directory.to_string_lossy());
        let stdout = util::invoke_cargo(
            "run",
            &[
                "--package",
                "metadata-gen",
                &manifest_path.cargo_arg()?,
                &target_dir_arg,
                "--release",
            ],
            manifest_path.directory(),
            vec![],
        )?;

        let mut near_abi: AbiRoot = serde_json::from_slice(&stdout)?;
        let metainfo = extract_metainfo(&crate_metadata);
        near_abi.metainfo = metainfo;
        let near_abi_json = serde_json::to_string_pretty(&near_abi)?;
        fs::write(&out_path_metadata, near_abi_json)?;

        Ok(())
    };

    Workspace::new(&crate_metadata.cargo_meta, &crate_metadata.root_package.id)?
        .with_root_package_manifest(|manifest| {
            manifest.with_added_crate_type("rlib")?.with_profile_release_lto(false)?;
            Ok(())
        })?
        .with_metadata_gen_package(crate_metadata.manifest_path.absolute_directory()?)?
        .using_temp(generate_metadata)?;

    Ok(MetadataResult { dest_metadata: out_path_metadata })
}
