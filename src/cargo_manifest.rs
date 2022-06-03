use std::path::{Path, PathBuf};

/// Path to a `Cargo.toml` file
#[derive(Clone, Debug)]
pub struct CargoManifest {
    path: PathBuf,
}

impl CargoManifest {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        CargoManifest {
            path: path.as_ref().into(),
        }
    }

    pub fn cargo_arg(&self) -> anyhow::Result<String> {
        let path = self
            .path
            .canonicalize()
            .map_err(|err| anyhow::anyhow!("Failed to canonicalize {:?}: {:?}", self.path, err))?;
        Ok(format!("--manifest-path={}", path.to_string_lossy()))
    }

    pub fn directory(&self) -> Option<&Path> {
        self.path.parent()
    }

    pub fn absolute_directory(&self) -> anyhow::Result<PathBuf> {
        let directory = match self.directory() {
            Some(dir) => dir,
            None => Path::new("./"),
        };
        directory
            .canonicalize()
            .map_err(|err| anyhow::anyhow!("Failed to canonicalize {:?}: {:?}", self.path, err))
    }
}

impl AsRef<Path> for CargoManifest {
    fn as_ref(&self) -> &Path {
        self.path.as_ref()
    }
}

impl From<CargoManifest> for PathBuf {
    fn from(path: CargoManifest) -> Self {
        path.path
    }
}
