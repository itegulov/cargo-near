use super::metadata;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::convert::TryFrom;
use std::fs;
use std::path::{Path, PathBuf};
use toml::value;

const MANIFEST_FILE: &str = "Cargo.toml";
const METADATA_PACKAGE_PATH: &str = ".near/metadata_gen";

/// Path to a `Cargo.toml` file
#[derive(Clone, Debug)]
pub struct ManifestPath {
    path: PathBuf,
}

impl ManifestPath {
    /// Create a new [`ManifestPath`], errors if not path to `Cargo.toml`
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let manifest = path.as_ref();
        if let Some(file_name) = manifest.file_name() {
            if file_name != MANIFEST_FILE {
                anyhow::bail!("Manifest file must be a Cargo.toml")
            }
        }
        Ok(ManifestPath { path: manifest.into() })
    }

    /// Create an arg `--manifest-path=` for `cargo` command
    pub fn cargo_arg(&self) -> Result<String> {
        let path = self
            .path
            .canonicalize()
            .map_err(|err| anyhow::anyhow!("Failed to canonicalize {:?}: {:?}", self.path, err))?;
        Ok(format!("--manifest-path={}", path.to_string_lossy()))
    }

    /// The directory path of the manifest path.
    ///
    /// Returns `None` if the path is just the plain file name `Cargo.toml`
    pub fn directory(&self) -> Option<&Path> {
        let just_a_file_name =
            self.path.iter().collect::<Vec<_>>() == vec![Path::new(MANIFEST_FILE)];
        if !just_a_file_name {
            self.path.parent()
        } else {
            None
        }
    }

    /// Returns the absolute directory path of the manifest.
    pub fn absolute_directory(&self) -> Result<PathBuf, std::io::Error> {
        let directory = match self.directory() {
            Some(dir) => dir,
            None => Path::new("./"),
        };
        directory.canonicalize()
    }
}

impl<P> TryFrom<Option<P>> for ManifestPath
where
    P: AsRef<Path>,
{
    type Error = anyhow::Error;

    fn try_from(value: Option<P>) -> Result<Self, Self::Error> {
        value.map_or(Ok(Default::default()), ManifestPath::new)
    }
}

impl Default for ManifestPath {
    fn default() -> ManifestPath {
        ManifestPath::new(MANIFEST_FILE).expect("it's a valid manifest file")
    }
}

impl AsRef<Path> for ManifestPath {
    fn as_ref(&self) -> &Path {
        self.path.as_ref()
    }
}

impl From<ManifestPath> for PathBuf {
    fn from(path: ManifestPath) -> Self {
        path.path
    }
}

/// Create, amend and save a copy of the specified `Cargo.toml`.
pub struct Manifest {
    path: ManifestPath,
    toml: value::Table,
    /// True if a metadata package should be generated for this manifest
    metadata_package: bool,
}

impl Manifest {
    /// Create new Manifest for the given manifest path.
    ///
    /// The path *must* be to a `Cargo.toml`.
    pub fn new(manifest_path: ManifestPath) -> Result<Manifest> {
        let toml = fs::read_to_string(&manifest_path).context("Loading Cargo.toml")?;
        let toml: value::Table = toml::from_str(&toml)?;

        Ok(Manifest { path: manifest_path, toml, metadata_package: false })
    }

    /// Get the path of the manifest file
    pub(super) fn path(&self) -> &ManifestPath {
        &self.path
    }

    /// Get mutable reference to `[lib] crate-types = []` section
    fn get_crate_types_mut(&mut self) -> Result<&mut value::Array> {
        let lib =
            self.toml.get_mut("lib").ok_or_else(|| anyhow::anyhow!("lib section not found"))?;
        let crate_types = lib
            .get_mut("crate-type")
            .ok_or_else(|| anyhow::anyhow!("crate-type section not found"))?;

        crate_types.as_array_mut().ok_or_else(|| anyhow::anyhow!("crate-types should be an Array"))
    }

    /// Add a value to the `[lib] crate-types = []` section.
    ///
    /// If the value already exists, does nothing.
    pub fn with_added_crate_type(&mut self, crate_type: &str) -> Result<&mut Self> {
        let crate_types = self.get_crate_types_mut()?;
        if !crate_type_exists(crate_type, crate_types) {
            crate_types.push(crate_type.into());
        }
        Ok(self)
    }

    /// Set `[profile.release]` lto flag
    pub fn with_profile_release_lto(&mut self, enabled: bool) -> Result<&mut Self> {
        let lto = self.get_profile_release_table_mut()?.entry("lto").or_insert(enabled.into());
        *lto = enabled.into();
        Ok(self)
    }

    /// Get mutable reference to `[profile.release]` section
    fn get_profile_release_table_mut(&mut self) -> Result<&mut value::Table> {
        let profile = self.toml.entry("profile").or_insert(value::Value::Table(Default::default()));
        let release = profile
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("profile should be a table"))?
            .entry("release")
            .or_insert(value::Value::Table(Default::default()));
        release.as_table_mut().ok_or_else(|| anyhow::anyhow!("release should be a table"))
    }

    /// Adds a metadata package to the manifest workspace for generating metadata
    pub fn with_metadata_package(&mut self) -> Result<&mut Self> {
        let workspace =
            self.toml.entry("workspace").or_insert(value::Value::Table(Default::default()));
        let members = workspace
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("workspace should be a table"))?
            .entry("members")
            .or_insert(value::Value::Array(Default::default()))
            .as_array_mut()
            .ok_or_else(|| anyhow::anyhow!("members should be an array"))?;
        members.push(METADATA_PACKAGE_PATH.into());

        self.metadata_package = true;
        Ok(self)
    }

    /// Replace relative paths with absolute paths with the working directory.
    ///
    /// Enables the use of a temporary amended copy of the manifest.
    ///
    /// # Rewrites
    ///
    /// - `[lib]/path`
    /// - `[dependencies]`
    ///
    /// Dependencies with package names specified in `exclude_deps` will not be rewritten.
    pub(super) fn rewrite_relative_paths<I, S>(&mut self, exclude_deps: I) -> Result<&mut Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let abs_path = self.path.as_ref().canonicalize()?;
        let abs_dir =
            abs_path.parent().expect("The manifest path is a file path so has a parent; qed");

        let to_absolute = |value_id: String, existing_path: &mut value::Value| -> Result<()> {
            let path_str = existing_path
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("{} should be a string", value_id))?;
            #[cfg(windows)]
            // On Windows path separators are `\`, hence we need to replace the `/` in
            // e.g. `src/lib.rs`.
            let path_str = &path_str.replace("/", "\\");
            let path = PathBuf::from(path_str);
            if path.is_relative() {
                let lib_abs = abs_dir.join(path);
                log::debug!("Rewriting {} to '{}'", value_id, lib_abs.display());
                *existing_path = value::Value::String(lib_abs.to_string_lossy().into())
            }
            Ok(())
        };

        let rewrite_path = |table_value: &mut value::Value, table_section: &str, default: &str| {
            let table = table_value.as_table_mut().ok_or_else(|| {
                anyhow::anyhow!("'[{}]' section should be a table", table_section)
            })?;

            match table.get_mut("path") {
                Some(existing_path) => {
                    to_absolute(format!("[{}]/path", table_section), existing_path)
                }
                None => {
                    let default_path = PathBuf::from(default);
                    if !default_path.exists() {
                        anyhow::bail!(
                            "No path specified, and the default `{}` was not found",
                            default
                        )
                    }
                    let path = abs_dir.join(default_path);
                    log::debug!("Adding default path '{}'", path.display());
                    table
                        .insert("path".into(), value::Value::String(path.to_string_lossy().into()));
                    Ok(())
                }
            }
        };

        // Rewrite `[lib] path = /path/to/lib.rs`
        if let Some(lib) = self.toml.get_mut("lib") {
            rewrite_path(lib, "lib", "src/lib.rs")?;
        }

        // Rewrite `[[bin]] path = /path/to/main.rs`
        if let Some(bin) = self.toml.get_mut("bin") {
            let bins = bin
                .as_array_mut()
                .ok_or_else(|| anyhow::anyhow!("'[[bin]]' section should be a table array"))?;

            // Rewrite `[[bin]] path =` value to an absolute path.
            for bin in bins {
                rewrite_path(bin, "[bin]", "src/main.rs")?;
            }
        }

        // Rewrite any dependency relative paths
        if let Some(dependencies) = self.toml.get_mut("dependencies") {
            let exclude =
                exclude_deps.into_iter().map(|s| s.as_ref().to_string()).collect::<HashSet<_>>();
            let table = dependencies
                .as_table_mut()
                .ok_or_else(|| anyhow::anyhow!("dependencies should be a table"))?;
            for (name, value) in table {
                let package_name = {
                    let package = value.get("package");
                    let package_name = package.and_then(|p| p.as_str()).unwrap_or(name);
                    package_name.to_string()
                };

                if !exclude.contains(&package_name) {
                    if let Some(dependency) = value.as_table_mut() {
                        if let Some(dep_path) = dependency.get_mut("path") {
                            to_absolute(format!("dependency {}", package_name), dep_path)?;
                        }
                    }
                }
            }
        }

        Ok(self)
    }

    /// Writes the amended manifest to the given path.
    pub fn write(&self, manifest_path: &ManifestPath) -> Result<()> {
        if let Some(dir) = manifest_path.directory() {
            fs::create_dir_all(dir).context(format!("Creating directory '{}'", dir.display()))?;
        }

        if self.metadata_package {
            let dir = if let Some(manifest_dir) = manifest_path.directory() {
                manifest_dir.join(METADATA_PACKAGE_PATH)
            } else {
                METADATA_PACKAGE_PATH.into()
            };

            fs::create_dir_all(&dir).context(format!("Creating directory '{}'", dir.display()))?;

            let contract_package_name = self
                .toml
                .get("package")
                .ok_or_else(|| anyhow::anyhow!("package section not found"))?
                .get("name")
                .ok_or_else(|| anyhow::anyhow!("[package] name field not found"))?
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("[package] name should be a string"))?;

            let near_sdk = self
                .toml
                .get("dependencies")
                .ok_or_else(|| anyhow::anyhow!("[dependencies] section not found"))?
                .get("near-sdk")
                .ok_or_else(|| anyhow::anyhow!("near-sdk dependency not found"))?
                .as_table()
                .ok_or_else(|| anyhow::anyhow!("near-sdk dependency should be a table"))?;

            metadata::generate_package(dir, contract_package_name, near_sdk.clone())?;
        }

        let updated_toml = toml::to_string(&self.toml)?;
        log::debug!("Writing updated manifest to '{}'", manifest_path.as_ref().display());
        fs::write(manifest_path, updated_toml)?;
        Ok(())
    }
}

fn crate_type_exists(crate_type: &str, crate_types: &[value::Value]) -> bool {
    crate_types.iter().any(|v| v.as_str().map_or(false, |s| s == crate_type))
}
