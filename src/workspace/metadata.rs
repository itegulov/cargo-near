use anyhow::Result;
use std::{collections::HashSet, fs, path::Path};
use toml::value;

/// Generates a cargo workspace package `metadata-gen` which will be invoked via `cargo run` to
/// generate contract metadata.
///
/// # Note
///
/// `near-sdk` dependencies are copied from the containing contract workspace to ensure the same
/// versions are utilized.
pub(super) fn generate_package<P: AsRef<Path>>(
    target_dir: P,
    contract_package_name: &str,
    mut near_sdk_dependency: value::Table,
    near_abi_symbols: &HashSet<String>,
) -> Result<()> {
    let dir = target_dir.as_ref();
    log::debug!(
        "Generating metadata package for {} in {}",
        contract_package_name,
        dir.display()
    );

    let extern_function_defs: String = near_abi_symbols
        .iter()
        .map(|sym_name| format!("fn {}() -> near_sdk::__private::AbiRoot;\n", sym_name))
        .collect();

    let abi_calls: String = near_abi_symbols
        .iter()
        .map(|sym_name| format!("unsafe {{ {}() }}, ", sym_name))
        .collect();
    let abi_calls = format!("vec![{}]", abi_calls);

    let cargo_toml = include_str!("../../templates/tools/generate-metadata/_Cargo.toml");
    let main_rs = include_str!("../../templates/tools/generate-metadata/main.rs");
    let main_rs = main_rs.replace("::extern_function_defs::", &extern_function_defs);
    let main_rs = main_rs.replace("::abi_calls::", &abi_calls);

    log::debug!("main.rs contents:\n{}", &main_rs);

    let mut cargo_toml: value::Table = toml::from_str(cargo_toml)?;
    let deps = cargo_toml
        .get_mut("dependencies")
        .expect("[dependencies] section specified in the template")
        .as_table_mut()
        .expect("[dependencies] is a table specified in the template");

    // initialize contract dependency
    let contract = deps
        .get_mut("contract")
        .expect("contract dependency specified in the template")
        .as_table_mut()
        .expect("contract dependency is a table specified in the template");
    contract.insert("package".into(), contract_package_name.into());

    // make near-sdk dependency use default features
    near_sdk_dependency.remove("default-features");
    near_sdk_dependency.remove("features");
    near_sdk_dependency.remove("optional");

    // add near-sdk dependencies copied from contract manifest
    deps.insert("near-sdk".into(), near_sdk_dependency.into());
    let cargo_toml = toml::to_string(&cargo_toml)?;

    log::debug!("Cargo.toml contents:\n{}", &cargo_toml);

    fs::write(dir.join("Cargo.toml"), cargo_toml)?;
    fs::write(dir.join("main.rs"), main_rs)?;
    Ok(())
}
