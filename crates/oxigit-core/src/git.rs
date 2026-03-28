use std::path::Path;
use std::process::Command;

use crate::error::{OxigitError, Result};

/// Initialize a bare git repository at the given path.
pub fn init_bare_repo(path: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["init", "--bare"])
        .arg(path)
        .output()?;

    if !output.status.success() {
        return Err(OxigitError::Git(format!(
            "Failed to init bare repo: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

/// Resolve the on-disk path to a bare repository.
pub fn repo_path(data_dir: &Path, owner: &str, name: &str) -> std::path::PathBuf {
    data_dir.join("repos").join(owner).join(format!("{name}.git"))
}
