use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use crate::error::{PatrolError, Result};

#[derive(Debug, Clone)]
pub struct GitCommandOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl GitCommandOutput {
    pub fn ok(&self) -> bool {
        self.status.success()
    }
}

pub fn run_git(repo: &Path, timeout_secs: u64, args: &[&str]) -> Result<GitCommandOutput> {
    let output = Command::new("timeout")
        .arg(timeout_secs.to_string())
        .arg("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| PatrolError::GitCommand {
            repo: repo.display().to_string(),
            message: format!("failed to spawn git with args {:?}: {}", args, err),
        })?;

    if output.status.code() == Some(124) {
        return Err(PatrolError::GitTimeout {
            repo: repo.display().to_string(),
            command: format!("git {}", args.join(" ")),
        });
    }

    Ok(GitCommandOutput {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

pub fn run_git_expect_stdout(repo: &Path, timeout_secs: u64, args: &[&str]) -> Result<String> {
    let out = run_git(repo, timeout_secs, args)?;
    if !out.ok() {
        return Err(PatrolError::GitCommand {
            repo: repo.display().to_string(),
            message: format!(
                "git {} failed: {}",
                args.join(" "),
                if out.stderr.is_empty() {
                    out.stdout.clone()
                } else {
                    out.stderr.clone()
                }
            ),
        });
    }
    Ok(out.stdout)
}
