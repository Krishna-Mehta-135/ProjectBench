use sha2::{Digest, Sha256};
use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("artifact path must be relative")]
    AbsolutePath,
    #[error("artifact path escapes the configured root")]
    PathEscape,
    #[error("artifact I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("artifact hash does not match persisted metadata")]
    HashMismatch,
}

#[derive(Debug, Clone)]
pub struct LocalArtifactStore {
    root: PathBuf,
}

impl LocalArtifactStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn put(&self, relative_path: &Path, content: &[u8]) -> Result<String, ArtifactError> {
        let path = self.safe_path(relative_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
        Ok(format!("sha256:{:x}", Sha256::digest(content)))
    }

    pub fn read(&self, relative_path: &Path) -> Result<Vec<u8>, ArtifactError> {
        Ok(fs::read(self.safe_path(relative_path)?)?)
    }

    pub fn read_verified(
        &self,
        relative_path: &Path,
        expected_sha256: &str,
    ) -> Result<Vec<u8>, ArtifactError> {
        let content = self.read(relative_path)?;
        let actual = format!("sha256:{:x}", Sha256::digest(&content));
        if actual != expected_sha256 {
            return Err(ArtifactError::HashMismatch);
        }
        Ok(content)
    }

    fn safe_path(&self, relative_path: &Path) -> Result<PathBuf, ArtifactError> {
        if relative_path.is_absolute() {
            return Err(ArtifactError::AbsolutePath);
        }
        if relative_path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(ArtifactError::PathEscape);
        }
        Ok(self.root.join(relative_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn stores_and_hashes_bytes() {
        let root = std::env::temp_dir().join(format!("metrics-artifacts-{}", Uuid::new_v4()));
        let store = LocalArtifactStore::new(&root);
        let hash = store.put(Path::new("run/raw.json"), b"{}").unwrap();
        assert!(hash.starts_with("sha256:"));
        assert_eq!(store.read(Path::new("run/raw.json")).unwrap(), b"{}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_path_escape() {
        let store = LocalArtifactStore::new(std::env::temp_dir());
        assert!(matches!(
            store.put(Path::new("../secret"), b"x"),
            Err(ArtifactError::PathEscape)
        ));
    }

    #[test]
    fn rejects_tampered_bytes() {
        let root = std::env::temp_dir().join(format!("metrics-artifacts-{}", Uuid::new_v4()));
        let store = LocalArtifactStore::new(&root);
        let hash = store.put(Path::new("run/raw.json"), b"original").unwrap();
        fs::write(root.join("run/raw.json"), b"tampered").unwrap();
        assert!(matches!(
            store.read_verified(Path::new("run/raw.json"), &hash),
            Err(ArtifactError::HashMismatch)
        ));
        let _ = fs::remove_dir_all(root);
    }
}
