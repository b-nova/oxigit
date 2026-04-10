use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use tracing;

use crate::error::{OxigitError, Result};

const SYSTEM_AUTHOR_NAME: &str = "Oxigit";
const SYSTEM_AUTHOR_EMAIL: &str = "noreply@oxigit";
const MAX_NEW_COMMITS: &str = "100";

trait SystemGitIdentity {
    fn system_identity(&mut self) -> &mut Self;
}

impl SystemGitIdentity for Command {
    fn system_identity(&mut self) -> &mut Self {
        self.env("GIT_AUTHOR_NAME", SYSTEM_AUTHOR_NAME)
            .env("GIT_AUTHOR_EMAIL", SYSTEM_AUTHOR_EMAIL)
            .env("GIT_COMMITTER_NAME", SYSTEM_AUTHOR_NAME)
            .env("GIT_COMMITTER_EMAIL", SYSTEM_AUTHOR_EMAIL)
    }
}

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

/// Resolve the on-disk path to a bare repository (legacy layout).
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
            .system_identity()
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

/// Squash a sequence of session commits into a single commit.
/// `shas` must be ordered oldest-first and must be contiguous at the branch tip.
/// Returns the new squashed commit SHA.
pub fn squash_session(
    repo_path: &Path,
    branch: &str,
    shas: &[String],
    message: &str,
) -> Result<String> {
    if shas.is_empty() {
        return Err(OxigitError::Git("No commits to squash".into()));
    }

    let branch_ref = format!("refs/heads/{}", branch);
    let tip_output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["rev-parse", &branch_ref])
        .output()?;
    if !tip_output.status.success() {
        return Err(OxigitError::Git(format!("Branch {} not found", branch)));
    }
    let tip = String::from_utf8_lossy(&tip_output.stdout).trim().to_string();

    let latest = &shas[shas.len() - 1];
    let earliest = &shas[0];

    // Verify latest session commit is at the branch tip
    if tip != *latest {
        return Err(OxigitError::Git(
            "Session commits must be at the branch tip to squash. Other commits exist after this session.".into()
        ));
    }

    // Get parent of earliest commit
    let parent_output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["rev-parse", &format!("{}^", earliest)])
        .output()?;
    if !parent_output.status.success() {
        return Err(OxigitError::Git("Cannot squash: earliest commit has no parent (root commit)".into()));
    }
    let base_parent = String::from_utf8_lossy(&parent_output.stdout).trim().to_string();

    // Get the tree of the latest commit (end state)
    let tree_output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["rev-parse", &format!("{}^{{tree}}", latest)])
        .output()?;
    if !tree_output.status.success() {
        return Err(OxigitError::Git("Failed to get tree for latest commit".into()));
    }
    let tree_sha = String::from_utf8_lossy(&tree_output.stdout).trim().to_string();

    // Create the squashed commit
    let commit = Command::new("git")
        .env("GIT_DIR", repo_path)
        .system_identity()
        .args(["commit-tree", &tree_sha, "-p", &base_parent, "-m", message])
        .output()?;
    if !commit.status.success() {
        return Err(OxigitError::Git("Failed to create squash commit".into()));
    }
    let new_sha = String::from_utf8_lossy(&commit.stdout).trim().to_string();

    // Update branch ref
    Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["update-ref", &branch_ref, &new_sha])
        .output()?;

    Ok(new_sha)
}

/// Cherry-pick a range of commits onto a target branch in a bare repo.
/// `shas` must be ordered oldest-first.
pub fn cherry_pick_range(
    repo_path: &Path,
    target_branch: &str,
    shas: &[String],
) -> Result<()> {
    if shas.is_empty() {
        return Ok(());
    }

    let branch_ref = format!("refs/heads/{}", target_branch);
    let tip_output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["rev-parse", &branch_ref])
        .output()?;
    if !tip_output.status.success() {
        return Err(OxigitError::Git(format!("Branch {} not found", target_branch)));
    }
    let mut current_tip = String::from_utf8_lossy(&tip_output.stdout).trim().to_string();

    let tmp_index = repo_path.join("tmp_index_cherrypick");

    for sha in shas {
        // Get parent of this commit
        let parent_output = Command::new("git")
            .env("GIT_DIR", repo_path)
            .args(["rev-parse", &format!("{}^", sha)])
            .output()?;
        if !parent_output.status.success() {
            continue; // Skip root commits
        }
        let parent = String::from_utf8_lossy(&parent_output.stdout).trim().to_string();

        // Three-way merge: base=parent, ours=current_tip, theirs=sha
        let read_tree = Command::new("git")
            .env("GIT_DIR", repo_path)
            .env("GIT_INDEX_FILE", &tmp_index)
            .args(["read-tree", "-m", "-i", &parent, &current_tip, sha])
            .output()?;
        if !read_tree.status.success() {
            let _ = std::fs::remove_file(&tmp_index);
            return Err(OxigitError::Git(format!(
                "Cherry-pick conflicts detected on commit {}",
                &sha[..7.min(sha.len())]
            )));
        }

        let write_tree = Command::new("git")
            .env("GIT_DIR", repo_path)
            .env("GIT_INDEX_FILE", &tmp_index)
            .args(["write-tree"])
            .output()?;
        if !write_tree.status.success() {
            let _ = std::fs::remove_file(&tmp_index);
            return Err(OxigitError::Git("Failed to write cherry-pick tree".into()));
        }
        let tree_sha = String::from_utf8_lossy(&write_tree.stdout).trim().to_string();

        // Get original commit message
        let msg_output = Command::new("git")
            .env("GIT_DIR", repo_path)
            .args(["log", "-1", "--format=%B", sha])
            .output()?;
        let original_msg = String::from_utf8_lossy(&msg_output.stdout).trim().to_string();

        let commit = Command::new("git")
            .env("GIT_DIR", repo_path)
            .system_identity()
            .args(["commit-tree", &tree_sha, "-p", &current_tip, "-m", &original_msg])
            .output()?;
        if !commit.status.success() {
            let _ = std::fs::remove_file(&tmp_index);
            return Err(OxigitError::Git("Failed to create cherry-pick commit".into()));
        }
        current_tip = String::from_utf8_lossy(&commit.stdout).trim().to_string();
    }

    let _ = std::fs::remove_file(&tmp_index);

    // Update branch ref
    Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["update-ref", &branch_ref, &current_tip])
        .output()?;

    Ok(())
}

/// Parse conflicting file paths from git merge-tree output.
fn parse_conflict_files(stdout: &str, stderr: &str) -> Vec<String> {
    let mut conflict_files = Vec::new();
    let combined = format!("{}\n{}", stdout, stderr);
    for line in combined.lines() {
        if let Some(rest) = line.strip_prefix("CONFLICT") {
            if let Some(idx) = rest.find("Merge conflict in ") {
                let path = rest[idx + "Merge conflict in ".len()..].trim().to_string();
                conflict_files.push(path);
            } else if let Some(idx) = rest.find("modify/delete: ") {
                let path = rest[idx + "modify/delete: ".len()..].trim();
                let path = path.split_whitespace().next().unwrap_or("").to_string();
                if !path.is_empty() {
                    conflict_files.push(path);
                }
            }
        }
    }
    conflict_files
}

/// Parse merge-tree output into (clean, conflict_files, tree_sha).
fn parse_merge_tree_output(output: &std::process::Output) -> (bool, Vec<String>, Option<String>) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        let tree_sha = stdout.lines().next().unwrap_or("").trim().to_string();
        (true, vec![], Some(tree_sha))
    } else {
        let lines: Vec<&str> = stdout.lines().collect();
        let tree_sha = lines.first().map(|l| l.trim().to_string());
        let conflict_files = parse_conflict_files(&stdout, &stderr);
        (false, conflict_files, tree_sha)
    }
}

/// Analyze whether reverting a single commit on top of a current ref produces conflicts.
/// Returns (clean, conflicting_file_paths, auto_merged_tree_sha).
///
/// Uses `git merge-tree --write-tree --merge-base=<commit>` to perform a real 3-way merge:
/// base=commit_sha, ours=current_ref, theirs=commit_sha^ (parent).
pub fn analyze_revert_commit(
    repo_path: &Path,
    commit_sha: &str,
    current_ref: &str,
) -> Result<(bool, Vec<String>, Option<String>)> {
    let parent = rev_parse(repo_path, &format!("{}^", commit_sha))?;

    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args([
            "merge-tree", "--write-tree",
            &format!("--merge-base={}", commit_sha),
            current_ref, &parent,
        ])
        .output()?;

    Ok(parse_merge_tree_output(&output))
}

/// Apply conflict resolutions and create a merge commit.
/// `resolutions` is a list of (file_path, resolved_content).
/// `parents` are the parent commit SHAs for the merge commit.
pub fn apply_conflict_resolutions(
    repo_path: &Path,
    auto_merged_tree: &str,
    resolutions: &[(String, String)],
    parents: &[&str],
    message: &str,
    branch: &str,
) -> Result<String> {
    use std::io::Write;

    let tmp_index = repo_path.join("tmp_index_conflict_resolve");

    // Read the auto-merged tree into temp index
    let read = Command::new("git")
        .env("GIT_DIR", repo_path)
        .env("GIT_INDEX_FILE", &tmp_index)
        .args(["read-tree", auto_merged_tree])
        .output()?;
    if !read.status.success() {
        let _ = std::fs::remove_file(&tmp_index);
        return Err(OxigitError::Git("Failed to read auto-merged tree".into()));
    }

    // Override each conflicted file with its resolution
    for (path, content) in resolutions {
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
            return Err(OxigitError::Git(format!("Failed to hash resolved content for {}", path)));
        }
        let blob_sha = String::from_utf8_lossy(&output.stdout).trim().to_string();

        let update = Command::new("git")
            .env("GIT_DIR", repo_path)
            .env("GIT_INDEX_FILE", &tmp_index)
            .args(["update-index", "--add", "--cacheinfo", &format!("100644,{},{}", blob_sha, path)])
            .output()?;
        if !update.status.success() {
            let _ = std::fs::remove_file(&tmp_index);
            return Err(OxigitError::Git(format!("Failed to update index for {}", path)));
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
        return Err(OxigitError::Git("Failed to write resolved tree".into()));
    }
    let tree_sha = String::from_utf8_lossy(&write_tree.stdout).trim().to_string();

    // Build commit-tree args with multiple parents
    let mut args = vec!["commit-tree".to_string(), tree_sha.clone()];
    for parent in parents {
        args.push("-p".to_string());
        args.push(parent.to_string());
    }
    args.push("-m".to_string());
    args.push(message.to_string());

    let commit = Command::new("git")
        .env("GIT_DIR", repo_path)
        .system_identity()
        .args(&args)
        .output()?;

    if !commit.status.success() {
        return Err(OxigitError::Git("Failed to create resolved merge commit".into()));
    }
    let commit_sha = String::from_utf8_lossy(&commit.stdout).trim().to_string();

    // Update branch ref
    let branch_ref = format!("refs/heads/{}", branch);
    Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["update-ref", &branch_ref, &commit_sha])
        .output()?;

    Ok(commit_sha)
}

// --- git blame ---

/// A single line of blame output.
#[derive(Debug, Clone, Serialize)]
pub struct BlameLine {
    pub line_number: usize,
    pub commit_sha: String,
    pub author: String,
    pub time: String,
    pub content: String,
}

/// Run git blame on a file and return per-line attribution.
pub fn blame_file(repo_path: &Path, git_ref: &str, file_path: &str) -> Result<Vec<BlameLine>> {
    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["blame", "--porcelain", git_ref, "--", file_path])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(OxigitError::Git(format!("git blame failed: {}", stderr.trim())));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = Vec::new();
    let mut current_sha = String::new();
    let mut current_author = String::new();
    let mut current_time = String::new();
    let mut current_line_no: usize = 0;

    // Porcelain format: blocks start with "<sha> <orig_line> <final_line> [<num_lines>]"
    // Followed by header lines, then "\t<content>"
    for raw_line in stdout.lines() {
        if let Some(content) = raw_line.strip_prefix('\t') {
            // This is a content line — emit the blame entry
            lines.push(BlameLine {
                line_number: current_line_no,
                commit_sha: current_sha.clone(),
                author: current_author.clone(),
                time: current_time.clone(),
                content: content.to_string(),
            });
        } else if raw_line.starts_with("author ") {
            current_author = raw_line[7..].to_string();
        } else if raw_line.starts_with("author-time ") {
            // Convert epoch to readable format
            let epoch = raw_line[12..].trim();
            if let Ok(ts) = epoch.parse::<i64>() {
                // Simple ISO-ish format without pulling in chrono
                current_time = format_epoch(ts);
            } else {
                current_time = epoch.to_string();
            }
        } else if raw_line.len() >= 40 && raw_line.as_bytes().iter().take(40).all(|b| b.is_ascii_hexdigit()) {
            // SHA line: "<sha> <orig_line> <final_line> [<num_lines>]"
            let parts: Vec<&str> = raw_line.splitn(4, ' ').collect();
            if parts.len() >= 3 {
                current_sha = parts[0].to_string();
                current_line_no = parts[2].parse().unwrap_or(0);
            }
        }
    }

    Ok(lines)
}

/// Simple epoch-to-date formatter (avoids chrono dependency).
fn format_epoch(epoch: i64) -> String {
    // Rough conversion — good enough for display
    let secs_per_day: i64 = 86400;
    let days_since_epoch = epoch / secs_per_day;
    let time_of_day = epoch % secs_per_day;

    // Compute year/month/day from days since 1970-01-01
    let mut days = days_since_epoch;
    let mut year = 1970i64;

    loop {
        let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 366 } else { 365 };
        if days < days_in_year { break; }
        days -= days_in_year;
        year += 1;
    }

    let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = [31, if is_leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 0;
    for (i, &md) in month_days.iter().enumerate() {
        if days < md as i64 { month = i + 1; break; }
        days -= md as i64;
    }
    let day = days + 1;

    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;

    format!("{:04}-{:02}-{:02} {:02}:{:02}", year, month, day, hours, minutes)
}

// --- .oxigit/context.json AI metadata ---

#[derive(Debug, Clone, Deserialize)]
pub struct OxigitContext {
    pub tool: String,
    pub model: Option<String>,
    pub session_id: Option<String>,
    pub prompt: Option<String>,
    pub prompt_index: Option<i64>,
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
    // Deserialize, then ensure prompt_index defaults to None if not present in JSON
    match serde_json::from_str::<OxigitContext>(&content) {
        Ok(ctx) => Ok(Some(ctx)),
        Err(e) => {
            tracing::warn!("Malformed .oxigit/context.json in commit {}: {}", sha, e);
            Ok(None)
        }
    }
}

/// Read AI metadata from git trailers in a commit message.
/// Returns `Ok(None)` if no `Oxigit-Tool` trailer is found.
pub fn read_oxigit_trailers(repo_path: &Path, sha: &str) -> Result<Option<OxigitContext>> {
    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["log", "-1", "--format=%B", sha])
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let body = String::from_utf8_lossy(&output.stdout);

    fn trailer_value<'a>(body: &'a str, key: &str) -> Option<String> {
        for line in body.lines().rev() {
            if let Some(val) = line.strip_prefix(key) {
                let val = val.trim();
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
        None
    }

    let tool = match trailer_value(&body, "Oxigit-Tool:") {
        Some(t) => t,
        None => return Ok(None),
    };

    let prompt_index = trailer_value(&body, "Oxigit-Prompt-Index:")
        .and_then(|v| v.parse::<i64>().ok());

    Ok(Some(OxigitContext {
        tool,
        model: trailer_value(&body, "Oxigit-Model:"),
        session_id: trailer_value(&body, "Oxigit-Session:"),
        prompt: trailer_value(&body, "Oxigit-Prompt:"),
        prompt_index,
    }))
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
        .args(["rev-list", &range, &format!("--max-count={MAX_NEW_COMMITS}")])
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
    args.push(format!("--max-count={MAX_NEW_COMMITS}"));

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

/// Get the aggregate diff for a session (diff between parent of earliest commit and latest commit).
/// `shas` must be ordered oldest-first.
pub fn session_aggregate_diff(repo_path: &Path, shas: &[String]) -> Result<String> {
    if shas.is_empty() {
        return Ok(String::new());
    }

    let earliest = &shas[0];
    let latest = &shas[shas.len() - 1];

    // Get parent of earliest commit
    let parent_output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["rev-parse", &format!("{}^", earliest)])
        .output()?;

    let diff = if parent_output.status.success() {
        let parent = String::from_utf8_lossy(&parent_output.stdout).trim().to_string();
        let output = Command::new("git")
            .env("GIT_DIR", repo_path)
            .args(["diff", &parent, latest])
            .output()?;
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        // Root commit — show full diff
        let output = Command::new("git")
            .env("GIT_DIR", repo_path)
            .args(["diff-tree", "-p", "--root", latest])
            .output()?;
        String::from_utf8_lossy(&output.stdout).to_string()
    };

    Ok(diff)
}

/// Get diff stats (lines added, lines deleted) for a session's commits.
/// `shas` must be ordered oldest-first. Same range logic as `session_aggregate_diff`.
pub fn session_diff_stats(repo_path: &Path, shas: &[String]) -> Result<(usize, usize)> {
    if shas.is_empty() {
        return Ok((0, 0));
    }

    let earliest = &shas[0];
    let latest = &shas[shas.len() - 1];

    let parent_output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["rev-parse", &format!("{}^", earliest)])
        .output()?;

    let numstat_output = if parent_output.status.success() {
        let parent = String::from_utf8_lossy(&parent_output.stdout).trim().to_string();
        Command::new("git")
            .env("GIT_DIR", repo_path)
            .args(["diff", "--numstat", &parent, latest])
            .output()?
    } else {
        // Root commit
        Command::new("git")
            .env("GIT_DIR", repo_path)
            .args(["diff-tree", "--numstat", "--root", latest])
            .output()?
    };

    let stdout = String::from_utf8_lossy(&numstat_output.stdout);
    let mut added = 0usize;
    let mut deleted = 0usize;

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            // Binary files show "-" instead of numbers
            if let Ok(a) = parts[0].parse::<usize>() {
                added += a;
            }
            if let Ok(d) = parts[1].parse::<usize>() {
                deleted += d;
            }
        }
    }

    Ok((added, deleted))
}

/// Resolve a git revision to its full SHA.
pub fn rev_parse(repo_path: &Path, rev: &str) -> Result<String> {
    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["rev-parse", rev])
        .output()?;
    if !output.status.success() {
        return Err(OxigitError::Git(format!("Failed to resolve rev: {}", rev)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Check if a session has been reverted by searching for revert commits.
pub fn is_session_reverted(repo_path: &Path, session_id: &str) -> bool {
    let short_id = &session_id[..8.min(session_id.len())];
    let grep_pattern = format!("Revert AI session {}", short_id);

    let output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["log", "--all", "--oneline", "--grep", &grep_pattern])
        .output();

    match output {
        Ok(o) => !String::from_utf8_lossy(&o.stdout).trim().is_empty(),
        Err(_) => false,
    }
}

/// Result of a revert operation that may encounter conflicts.
#[derive(Debug)]
pub enum RevertResult {
    /// All commits reverted successfully.
    Success,
    /// Conflict detected at a specific commit.
    Conflict {
        /// SHA of the commit that caused the conflict.
        conflicting_sha: String,
        /// SHAs already successfully reverted (in revert order, newest-first).
        reverted_shas: Vec<String>,
        /// SHAs remaining to be reverted, including the conflicting one (oldest-first).
        remaining_shas: Vec<String>,
        /// The current commit after successful partial reverts (or original tip).
        current_commit: String,
        /// Auto-merged tree SHA from merge-tree (may contain conflict markers).
        auto_tree: Option<String>,
        /// File paths with conflicts.
        conflict_files: Vec<String>,
    },
}

/// Revert a sequence of commits on a bare repo, creating a single revert commit.
/// `shas` should be ordered oldest-first; they are reverted newest-first.
/// Returns `RevertResult::Conflict` if a commit cannot be cleanly reverted.
pub fn revert_session(repo_path: &Path, branch: &str, shas: &[String], message: &str) -> Result<RevertResult> {
    if shas.is_empty() {
        return Ok(RevertResult::Success);
    }

    let branch_ref = format!("refs/heads/{}", branch);
    let tip_output = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["rev-parse", &branch_ref])
        .output()?;
    if !tip_output.status.success() {
        return Err(OxigitError::Git(format!("Branch {} not found", branch)));
    }
    let original_tip = String::from_utf8_lossy(&tip_output.stdout).trim().to_string();
    let mut current = original_tip.clone();
    let mut reverted_shas = Vec::new();

    // Revert each commit newest-first
    for (rev_idx, sha) in shas.iter().rev().enumerate() {
        // Check for conflicts before attempting the revert
        let (clean, conflict_files, auto_tree) = analyze_revert_commit(repo_path, sha, &current)?;
        if !clean {
            // Remaining SHAs: from the conflicting commit back to the oldest (oldest-first order)
            let remaining: Vec<String> = shas[..(shas.len() - rev_idx)].to_vec();
            return Ok(RevertResult::Conflict {
                conflicting_sha: sha.clone(),
                reverted_shas,
                remaining_shas: remaining,
                current_commit: current,
                auto_tree,
                conflict_files,
            });
        }

        let parent = rev_parse(repo_path, &format!("{}^", sha))
            .map_err(|_| OxigitError::Git("Cannot find parent commit".into()))?;

        // Use merge-tree to produce the reverted tree (already verified clean by analyze_revert_commit)
        let merge_output = Command::new("git")
            .env("GIT_DIR", repo_path)
            .args([
                "merge-tree", "--write-tree",
                &format!("--merge-base={}", sha),
                &current, &parent,
            ])
            .output()?;
        if !merge_output.status.success() {
            return Err(OxigitError::Git(format!(
                "Cannot revert commit {}: merge-tree failed", &sha[..7.min(sha.len())]
            )));
        }
        let tree_sha = String::from_utf8_lossy(&merge_output.stdout)
            .lines().next().unwrap_or("").trim().to_string();

        let commit = Command::new("git")
            .env("GIT_DIR", repo_path)
            .system_identity()
            .args(["commit-tree", &tree_sha, "-p", &current, "-m", &format!("revert {}", sha)])
            .output()?;
        if !commit.status.success() {
            return Err(OxigitError::Git("Failed to create revert commit".into()));
        }
        current = String::from_utf8_lossy(&commit.stdout).trim().to_string();
        reverted_shas.push(sha.clone());
    }

    // Squash into single commit parented on original tip
    let final_tree = Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["rev-parse", &format!("{}^{{tree}}", current)])
        .output()?;
    let final_tree_sha = String::from_utf8_lossy(&final_tree.stdout).trim().to_string();

    let squash = Command::new("git")
        .env("GIT_DIR", repo_path)
        .system_identity()
        .args(["commit-tree", &final_tree_sha, "-p", &original_tip, "-m", message])
        .output()?;
    if !squash.status.success() {
        return Err(OxigitError::Git("Failed to create squashed revert commit".into()));
    }
    let squash_sha = String::from_utf8_lossy(&squash.stdout).trim().to_string();

    Command::new("git")
        .env("GIT_DIR", repo_path)
        .args(["update-ref", &branch_ref, &squash_sha])
        .output()?;

    Ok(RevertResult::Success)
}
