use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;

use pumpkinpi_protocol::{OperationId, ProjectId};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkspaceStatus {
    Active,
    Approved,
    Promoted,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkspaceRecord {
    pub project_id: ProjectId,
    pub operation_id: OperationId,
    pub primary_root: PathBuf,
    pub primary_cwd: PathBuf,
    pub worktree_root: PathBuf,
    pub execution_cwd: PathBuf,
    pub branch: String,
    pub base_commit: String,
    pub checkpoint_commit: String,
    pub status: WorkspaceStatus,
}

async fn git(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;
    if !output.status.success() {
        return Err(anyhow!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) async fn prepare(
    data_dir: &Path,
    project_id: &ProjectId,
    operation_id: &OperationId,
    project_cwd: &Path,
) -> Result<WorkspaceRecord> {
    let primary_root = PathBuf::from(git(project_cwd, &["rev-parse", "--show-toplevel"]).await?);
    let primary_root = tokio::fs::canonicalize(&primary_root).await?;
    let primary_cwd = tokio::fs::canonicalize(project_cwd).await?;
    let relative_cwd = primary_cwd
        .strip_prefix(&primary_root)
        .context("Project cwd is outside its Git repository")?;
    if !git(&primary_root, &["status", "--porcelain"])
        .await?
        .is_empty()
    {
        return Err(anyhow!(
            "primary Project worktree is dirty; checkpoint or clean it before autonomous realization"
        ));
    }
    let base_commit = git(&primary_root, &["rev-parse", "HEAD"]).await?;
    let branch = format!("pumpkinpi/{}", operation_id.0);
    let worktree_root = data_dir
        .join("worktrees")
        .join(&project_id.0)
        .join(&operation_id.0);
    if worktree_root.exists() {
        tokio::fs::remove_dir_all(&worktree_root).await?;
    }
    if let Some(parent) = worktree_root.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let worktree_text = worktree_root.to_string_lossy().into_owned();
    git(
        &primary_root,
        &[
            "worktree",
            "add",
            "-b",
            &branch,
            &worktree_text,
            &base_commit,
        ],
    )
    .await?;
    let execution_cwd = worktree_root.join(relative_cwd);
    Ok(WorkspaceRecord {
        project_id: project_id.clone(),
        operation_id: operation_id.clone(),
        primary_root,
        primary_cwd,
        worktree_root,
        execution_cwd,
        branch,
        base_commit: base_commit.clone(),
        checkpoint_commit: base_commit,
        status: WorkspaceStatus::Active,
    })
}

pub(crate) async fn verify(record: &WorkspaceRecord) -> Result<()> {
    if !record.worktree_root.is_dir() || !record.execution_cwd.is_dir() {
        return Err(anyhow!("isolated realization worktree is missing"));
    }
    let branch = git(&record.worktree_root, &["branch", "--show-current"]).await?;
    if branch != record.branch {
        return Err(anyhow!("isolated worktree branch changed unexpectedly"));
    }
    Ok(())
}

pub(crate) async fn checkpoint(record: &mut WorkspaceRecord, iteration: u64) -> Result<bool> {
    git(&record.worktree_root, &["add", "-A"]).await?;
    let changed = !git(&record.worktree_root, &["diff", "--cached", "--name-only"])
        .await?
        .is_empty();
    if changed {
        let message = format!("PumpkinPi realization iteration {iteration}");
        git(
            &record.worktree_root,
            &[
                "-c",
                "user.name=PumpkinPi",
                "-c",
                "user.email=pumpkinpi@localhost",
                "commit",
                "--no-gpg-sign",
                "-m",
                &message,
            ],
        )
        .await?;
    }
    record.checkpoint_commit = git(&record.worktree_root, &["rev-parse", "HEAD"]).await?;
    Ok(changed)
}

pub(crate) async fn rollback(record: &WorkspaceRecord) -> Result<()> {
    git(
        &record.worktree_root,
        &["reset", "--hard", &record.checkpoint_commit],
    )
    .await?;
    git(&record.worktree_root, &["clean", "-fd"]).await?;
    Ok(())
}

pub(crate) async fn promote(record: &mut WorkspaceRecord) -> Result<()> {
    verify(record).await?;
    if !git(&record.worktree_root, &["status", "--porcelain"])
        .await?
        .is_empty()
    {
        return Err(anyhow!("approved worktree has uncheckpointed changes"));
    }
    if !git(&record.primary_root, &["status", "--porcelain"])
        .await?
        .is_empty()
    {
        return Err(anyhow!(
            "primary Project changed during realization; promotion blocked"
        ));
    }
    let primary_head = git(&record.primary_root, &["rev-parse", "HEAD"]).await?;
    if primary_head != record.base_commit {
        return Err(anyhow!(
            "primary Project advanced from {} to {}; promotion requires reconciliation",
            record.base_commit,
            primary_head
        ));
    }
    record.status = WorkspaceStatus::Approved;
    git(
        &record.primary_root,
        &["merge", "--ff-only", &record.checkpoint_commit],
    )
    .await?;
    record.status = WorkspaceStatus::Promoted;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    async fn init_repo(root: &Path) {
        tokio::fs::create_dir_all(root).await.unwrap();
        git(root, &["init", "-b", "main"]).await.unwrap();
        tokio::fs::write(root.join("file.txt"), "base\n")
            .await
            .unwrap();
        git(root, &["add", "."]).await.unwrap();
        git(
            root,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-m",
                "base",
            ],
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn checkpoint_and_fast_forward_promotion_are_transactional() {
        let root = std::env::temp_dir().join(format!("pumpkinpi-workspace-{}", Uuid::new_v4()));
        let repo = root.join("repo");
        init_repo(&repo).await;
        let mut record = prepare(
            &root.join("state"),
            &ProjectId("project".into()),
            &OperationId("operation".into()),
            &repo,
        )
        .await
        .unwrap();
        tokio::fs::write(record.execution_cwd.join("file.txt"), "changed\n")
            .await
            .unwrap();
        assert!(checkpoint(&mut record, 1).await.unwrap());
        assert_eq!(
            tokio::fs::read_to_string(repo.join("file.txt"))
                .await
                .unwrap(),
            "base\n"
        );
        promote(&mut record).await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(repo.join("file.txt"))
                .await
                .unwrap(),
            "changed\n"
        );
        assert_eq!(record.status, WorkspaceStatus::Promoted);
        let _ = tokio::fs::remove_dir_all(root).await;
    }
}
