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

#[cfg(unix)]
fn build_git_command(repo: &Path, timeout_secs: u64, args: &[&str]) -> Command {
    let mut cmd = Command::new("timeout");
    cmd.arg(timeout_secs.to_string())
        .arg("git")
        .arg("-C")
        .arg(repo)
        .args(args);
    cmd
}

#[cfg(not(unix))]
fn build_git_command(repo: &Path, _timeout_secs: u64, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo).args(args);
    cmd
}

pub fn run_git(repo: &Path, timeout_secs: u64, args: &[&str]) -> Result<GitCommandOutput> {
    let output = build_git_command(repo, timeout_secs, args)
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
