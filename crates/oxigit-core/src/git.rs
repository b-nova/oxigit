use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use tracing;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
    pub author: String,
    pub time: String,
}

/// List branches in a repository using git command.
pub fn list_branches(repo_path: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["branch", "--format=%(refname:short)"])
        .output()?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().map(|s| s.to_string()).filter(|s| !s.is_empty()).collect())
}

/// Get the default branch (HEAD target or first branch).
pub fn default_branch(repo_path: &Path) -> Result<Option<String>> {
    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["symbolic-ref", "--short", "HEAD"])
        .output()?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !branch.is_empty() {
            return Ok(Some(branch));
        }
    }

    // Fallback: first branch
    let branches = list_branches(repo_path)?;
    Ok(branches.into_iter().next())
}

/// List files/directories in a tree at a given path and ref.
pub fn list_tree(repo_path: &Path, git_ref: &str, path: &str) -> Result<Vec<TreeEntry>> {
    // Use ls-tree to list entries
    let tree_path = if path.is_empty() {
        git_ref.to_string()
    } else {
        format!("{}:{}", git_ref, path)
    };

    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["ls-tree", "-l", &tree_path])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Not a valid object name") || stderr.contains("fatal:") {
            return Ok(vec![]);
        }
        return Err(OxigitError::Git(format!("ls-tree failed: {}", stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries: Vec<TreeEntry> = stdout
        .lines()
        .filter_map(|line| {
            // Format: <mode> <type> <hash> <size>\t<name>
            let (meta, name) = line.split_once('\t')?;
            let parts: Vec<&str> = meta.split_whitespace().collect();
            if parts.len() < 4 {
                return None;
            }
            let is_dir = parts[1] == "tree";
            let size = if is_dir {
                0
            } else {
                parts[3].parse().unwrap_or(0)
            };
            Some(TreeEntry {
                name: name.to_string(),
                is_dir,
                size,
            })
        })
        .collect();

    // Sort: directories first, then alphabetical
    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name))
    });

    Ok(entries)
}

/// Read the contents of a blob (file) at a given ref and path.
pub fn read_blob(repo_path: &Path, git_ref: &str, path: &str) -> Result<Vec<u8>> {
    let object = format!("{}:{}", git_ref, path);

    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["show", &object])
        .output()?;

    if !output.status.success() {
        return Err(OxigitError::NotFound(format!("File not found: {}", path)));
    }

    Ok(output.stdout)
}

/// Get the latest commit on a given ref.
pub fn get_latest_commit(repo_path: &Path, git_ref: &str) -> Result<Option<CommitInfo>> {
    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args([
            "log",
            "-1",
            "--format=%H%n%s%n%an%n%ai",
            git_ref,
        ])
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    if lines.len() < 4 {
        return Ok(None);
    }

    Ok(Some(CommitInfo {
        id: lines[0].to_string(),
        message: lines[1].to_string(),
        author: lines[2].to_string(),
        time: lines[3].to_string(),
    }))
}

/// List commits on a given ref, up to `limit`.
pub fn list_commits(repo_path: &Path, git_ref: &str, limit: usize) -> Result<Vec<CommitInfo>> {
    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args([
            "log",
            &format!("-{}", limit),
            "--format=%H%x00%s%x00%an%x00%ai",
            git_ref,
        ])
        .output()?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(4, '\0').collect();
            if parts.len() < 4 {
                return None;
            }
            Some(CommitInfo {
                id: parts[0].to_string(),
                message: parts[1].to_string(),
                author: parts[2].to_string(),
                time: parts[3].to_string(),
            })
        })
        .collect())
}

/// Show the diff for a single commit.
pub fn show_commit_diff(repo_path: &Path, sha: &str) -> Result<(CommitInfo, String)> {
    // Get commit info
    let commit = get_latest_commit(repo_path, sha)?
        .ok_or_else(|| OxigitError::NotFound("Commit not found".into()))?;

    // Get diff
    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["diff-tree", "-p", "--stat", "--root", sha])
        .output()?;

    let diff = if output.status.success() {
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        String::new()
    };

    Ok((commit, diff))
}

/// Get the diff between two branches (target...source).
pub fn branch_diff(repo_path: &Path, target: &str, source: &str) -> Result<String> {
    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["diff", &format!("{}...{}", target, source)])
        .output()?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// List commits between two branches (commits in source not in target).
pub fn branch_commits(repo_path: &Path, target: &str, source: &str) -> Result<Vec<CommitInfo>> {
    let range = format!("{}..{}", target, source);
    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args([
            "log",
            "--format=%H%x00%s%x00%an%x00%ai",
            &range,
        ])
        .output()?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(4, '\0').collect();
            if parts.len() < 4 {
                return None;
            }
            Some(CommitInfo {
                id: parts[0].to_string(),
                message: parts[1].to_string(),
                author: parts[2].to_string(),
                time: parts[3].to_string(),
            })
        })
        .collect())
}

/// Check if a branch can be fast-forward merged into another.
pub fn can_merge(repo_path: &Path, target: &str, source: &str) -> Result<bool> {
    // Check if merge-base exists and source is ahead of target
    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["merge-base", "--is-ancestor", target, source])
        .output()?;

    Ok(output.status.success())
}

/// Merge source branch into target branch.
/// Uses a temporary worktree for non-bare-repo merge operations.
pub fn merge_branches(repo_path: &Path, target: &str, source: &str, message: &str) -> Result<()> {
    // For bare repos, we use git update-ref for fast-forward merges
    // First check if it's a fast-forward
    let merge_base = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["merge-base", target, source])
        .output()?;

    let target_rev = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["rev-parse", target])
        .output()?;

    let source_rev = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["rev-parse", source])
        .output()?;

    let merge_base_sha = String::from_utf8_lossy(&merge_base.stdout).trim().to_string();
    let target_sha = String::from_utf8_lossy(&target_rev.stdout).trim().to_string();
    let source_sha = String::from_utf8_lossy(&source_rev.stdout).trim().to_string();

    if merge_base_sha == target_sha {
        // Fast-forward: just update the ref
        let output = Command::new("git")
            .env("GIT_DIR", repo_path)
            .args(["update-ref", &format!("refs/heads/{}", target), &source_sha])
            .output()?;

        if !output.status.success() {
            return Err(OxigitError::Git(format!(
                "Failed to fast-forward merge: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    } else {
        // Non-fast-forward: create a merge commit using plumbing commands
        // Read the trees
        let source_tree = Command::new("git")
            .env("GIT_DIR", repo_path)
            .args(["rev-parse", &format!("{}^{{tree}}", source)])
            .output()?;
        let _source_tree_sha = String::from_utf8_lossy(&source_tree.stdout).trim().to_string();

        // Try a tree merge
        let read_tree = Command::new("git")
            .env("GIT_DIR", repo_path)
            .args(["read-tree", "-m", "-i", &merge_base_sha, &target_sha, &source_sha])
            .output()?;

        if !read_tree.status.success() {
            return Err(OxigitError::Git("Merge conflicts detected. Cannot auto-merge.".into()));
        }

        // Write tree
        let write_tree = Command::new("git")
            .env("GIT_DIR", repo_path)
            .args(["write-tree"])
            .output()?;

        if !write_tree.status.success() {
            return Err(OxigitError::Git("Failed to write merge tree".into()));
        }
        let tree_sha = String::from_utf8_lossy(&write_tree.stdout).trim().to_string();

        // Create merge commit
        let commit = Command::new("git")
            .env("GIT_DIR", repo_path)
            .env("GIT_AUTHOR_NAME", "Oxigit")
            .env("GIT_AUTHOR_EMAIL", "noreply@oxigit")
            .env("GIT_COMMITTER_NAME", "Oxigit")
            .env("GIT_COMMITTER_EMAIL", "noreply@oxigit")
            .args([
                "commit-tree", &tree_sha,
                "-p", &target_sha,
                "-p", &source_sha,
                "-m", message,
            ])
            .output()?;

        if !commit.status.success() {
            return Err(OxigitError::Git(format!(
                "Failed to create merge commit: {}",
                String::from_utf8_lossy(&commit.stderr)
            )));
        }
        let merge_sha = String::from_utf8_lossy(&commit.stdout).trim().to_string();

        // Update target ref
        let update = Command::new("git")
            .env("GIT_DIR", repo_path)
            .args(["update-ref", &format!("refs/heads/{}", target), &merge_sha])
            .output()?;

        if !update.status.success() {
            return Err(OxigitError::Git("Failed to update ref after merge".into()));
        }

        Ok(())
    }
}

/// Check if a branch exists.
pub fn branch_exists(repo_path: &Path, branch: &str) -> bool {
    Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["rev-parse", "--verify", &format!("refs/heads/{}", branch)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Add files to a branch in a bare repo by creating a new commit.
/// Uses git plumbing (hash-object, mktree, commit-tree, update-ref).
/// `files` is a list of (path, content, executable) tuples.
pub fn add_files_to_branch(
    repo_path: &Path,
    branch: &str,
    files: &[(&str, &str, bool)],
    message: &str,
    author_name: &str,
    author_email: &str,
) -> Result<String> {
    use std::io::Write;

    let branch_ref = format!("refs/heads/{}", branch);

    // Get parent commit SHA
    let parent = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["rev-parse", &branch_ref])
        .output()?;
    if !parent.status.success() {
        return Err(OxigitError::Git(format!("Branch '{}' not found", branch)));
    }
    let parent_sha = String::from_utf8_lossy(&parent.stdout).trim().to_string();

    // Use a temp index so we don't disturb anything
    let tmp_index = repo_path.join("tmp_index_addfiles");

    // Read parent tree into temp index
    let read = Command::new("git")
        .env("GIT_DIR", repo_path)
        .env("GIT_INDEX_FILE", &tmp_index)
        .args(["read-tree", &parent_sha])
        .output()?;
    if !read.status.success() {
        let _ = std::fs::remove_file(&tmp_index);
        return Err(OxigitError::Git("Failed to read parent tree".into()));
    }

    // Hash each file and add to index
    for (path, content, executable) in files {
        let mut child = Command::new("git")
            .env("GIT_DIR", repo_path)
            .args(["hash-object", "-w", "--stdin"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        if let Some(ref mut stdin) = child.stdin {
            stdin.write_all(content.as_bytes())?;
        }
        let output = child.wait_with_output()?;
        if !output.status.success() {
            let _ = std::fs::remove_file(&tmp_index);
            return Err(OxigitError::Git(format!("Failed to hash object for {}", path)));
        }
        let blob_sha = String::from_utf8_lossy(&output.stdout).trim().to_string();

        let mode = if *executable { "100755" } else { "100644" };
        let update = Command::new("git")
            .env("GIT_DIR", repo_path)
            .env("GIT_INDEX_FILE", &tmp_index)
            .args(["update-index", "--add", "--cacheinfo", &format!("{},{},{}", mode, blob_sha, path)])
            .output()?;
        if !update.status.success() {
            let _ = std::fs::remove_file(&tmp_index);
            return Err(OxigitError::Git(format!(
                "Failed to update index for {}: {}",
                path,
                String::from_utf8_lossy(&update.stderr)
            )));
        }
    }

    // Write tree
    let write_tree = Command::new("git")
        .env("GIT_DIR", repo_path)
        .env("GIT_INDEX_FILE", &tmp_index)
        .args(["write-tree"])
        .output()?;
    let _ = std::fs::remove_file(&tmp_index);

    if !write_tree.status.success() {
        return Err(OxigitError::Git("Failed to write tree".into()));
    }
    let tree_sha = String::from_utf8_lossy(&write_tree.stdout).trim().to_string();

    // Create commit
    let commit = Command::new("git")
        .env("GIT_DIR", repo_path)
        .env("GIT_AUTHOR_NAME", author_name)
        .env("GIT_AUTHOR_EMAIL", author_email)
        .env("GIT_COMMITTER_NAME", author_name)
        .env("GIT_COMMITTER_EMAIL", author_email)
        .args(["commit-tree", &tree_sha, "-p", &parent_sha, "-m", message])
        .output()?;

    if !commit.status.success() {
        return Err(OxigitError::Git(format!(
            "Failed to create commit: {}",
            String::from_utf8_lossy(&commit.stderr)
        )));
    }
    let commit_sha = String::from_utf8_lossy(&commit.stdout).trim().to_string();

    // Update branch ref
    let update_ref = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["update-ref", &branch_ref, &commit_sha])
        .output()?;

    if !update_ref.status.success() {
        return Err(OxigitError::Git("Failed to update branch ref".into()));
    }

    Ok(commit_sha)
}

// --- .oxigit/context.json AI metadata ---

#[derive(Debug, Clone, Deserialize)]
pub struct OxigitContext {
    pub tool: String,
    pub model: Option<String>,
    pub session_id: Option<String>,
    pub prompt: Option<String>,
}

/// Read `.oxigit/context.json` from a specific commit in a bare repo.
/// Returns `Ok(None)` if the file doesn't exist at that commit.
pub fn read_oxigit_context(repo_path: &Path, sha: &str) -> Result<Option<OxigitContext>> {
    let object = format!("{}:.oxigit/context.json", sha);
    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["show", &object])
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let content = String::from_utf8_lossy(&output.stdout);
    match serde_json::from_str::<OxigitContext>(&content) {
        Ok(ctx) => Ok(Some(ctx)),
        Err(e) => {
            tracing::warn!("Malformed .oxigit/context.json in commit {}: {}", sha, e);
            Ok(None)
        }
    }
}

/// List files changed by a commit (auto-detected via diff-tree).
/// Excludes files under `.oxigit/`.
pub fn list_changed_files(repo_path: &Path, sha: &str) -> Result<Vec<String>> {
    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["diff-tree", "--root", "--name-only", "-r", "--no-commit-id", sha])
        .output()?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter(|s| !s.is_empty() && !s.starts_with(".oxigit/"))
        .map(|s| s.to_string())
        .collect())
}

/// Snapshot all refs in a repository. Returns a map of refname -> SHA.
pub fn capture_refs(repo_path: &Path) -> Result<HashMap<String, String>> {
    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["show-ref"])
        .output()?;

    // show-ref exits with 1 if no refs exist (empty repo), which is fine
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut refs = HashMap::new();
    for line in stdout.lines() {
        if let Some((sha, refname)) = line.split_once(' ') {
            refs.insert(refname.to_string(), sha.to_string());
        }
    }
    Ok(refs)
}

/// List commit SHAs in a range (old..new).
pub fn list_new_commit_shas(repo_path: &Path, old_sha: &str, new_sha: &str) -> Result<Vec<String>> {
    let range = format!("{}..{}", old_sha, new_sha);
    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["rev-list", &range, "--max-count=100"])
        .output()?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
}

/// List commit SHAs reachable from `new_sha` but not from any of the `exclude_shas`.
/// Used for new branches where there's no old SHA.
pub fn list_new_commit_shas_excluding(repo_path: &Path, new_sha: &str, exclude_shas: &[&str]) -> Result<Vec<String>> {
    let mut args = vec!["rev-list".to_string(), new_sha.to_string()];
    if !exclude_shas.is_empty() {
        args.push("--not".to_string());
        for sha in exclude_shas {
            args.push(sha.to_string());
        }
    }
    args.push("--max-count=100".to_string());

    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(&args)
        .output()?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
}
