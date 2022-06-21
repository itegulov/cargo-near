use crate::crate_metadata::CrateMetadata;
use crate::util;
use crate::workspace::{ManifestPath, Workspace};
use anyhow::Result;
use near_sdk::__private::{AbiMetadata, AbiRoot};
use std::collections::{HashMap, HashSet};
use std::{fs, path::PathBuf};

const ABI_FILE: &str = "abi.json";

/// ABI generation result.
#[derive(serde::Serialize)]
pub struct AbiResult {
    /// Path to the resulting ABI file.
    pub dest_abi: PathBuf,
}

fn extract_metadata(crate_metadata: &CrateMetadata) -> AbiMetadata {
    let package = &crate_metadata.root_package;
    AbiMetadata {
        name: Some(package.name.clone()),
        version: Some(package.version.to_string()),
        authors: package.authors.clone(),
        other: HashMap::new(),
    }
}

pub(crate) fn execute(crate_metadata: &CrateMetadata) -> Result<AbiResult> {
    let target_directory = crate_metadata.target_directory.clone();
    let out_path_abi = target_directory.join(ABI_FILE);

    let generate_abi = |manifest_path: &ManifestPath| -> Result<()> {
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
        let metadata = extract_metadata(&crate_metadata);
        near_abi.metadata = metadata;
        let near_abi_json = serde_json::to_string_pretty(&near_abi)?;
        fs::write(&out_path_abi, near_abi_json)?;

        Ok(())
    };

    let lib_file_path = util::compile_project(crate_metadata)?;
    let lib_file_contents = fs::read(&lib_file_path)?;
    let object = symbolic_debuginfo::Object::parse(&lib_file_contents)?;
    log::debug!(
        "A shared library was built at {:?} with format {} for architecture {}",
        &lib_file_path,
        &object.file_format(),
        &object.arch()
    );
    let near_abi_symbols = object
        .symbols()
        .flat_map(|sym| sym.name)
        .filter(|sym_name| sym_name.starts_with("__near_abi_"))
        .map(|sym_name| sym_name.to_string())
        .collect::<HashSet<_>>();
    log::debug!(
        "Detected following NEAR ABI symbols: {:?}",
        &near_abi_symbols
    );

    Workspace::new(
        &crate_metadata.cargo_meta,
        &crate_metadata.root_package.id,
        near_abi_symbols,
    )?
    .with_root_package_manifest(|manifest| {
        manifest
            .with_added_crate_type("rlib")?
            .with_profile_release_lto(false)?;
        Ok(())
    })?
    .with_metadata_gen_package(crate_metadata.manifest_path.absolute_directory()?)?
    .using_temp(generate_abi)?;

    Ok(AbiResult {
        dest_abi: out_path_abi,
    })
}
