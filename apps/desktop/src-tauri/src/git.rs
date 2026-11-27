mod process;
mod repository;
#[cfg(test)]
mod tests;

use nimblepost_core::CancellationToken;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Error {
    code: &'static str,
    message: &'static str,
}

impl Error {
    pub(crate) fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }

    fn unavailable() -> Self {
        Self::new(
            "executable-unavailable",
            "Git is unavailable. Install Git and make it available on PATH.",
        )
    }

    fn io() -> Self {
        Self::new("process-failed", "Git data could not be read or written.")
    }

    fn unsupported() -> Self {
        Self::new(
            "unsupported",
            "This repository layout or Git configuration is unsupported.",
        )
    }

    fn limit() -> Self {
        Self::new(
            "limit",
            "This repository or diff exceeds the Git panel limits.",
        )
    }

    fn invalid_path() -> Self {
        Self::new(
            "invalid-path",
            "Choose a regular file within this collection.",
        )
    }

    fn stale() -> Self {
        Self::new(
            "stale-state",
            "Git changed since the last review. Refresh and review the selection again.",
        )
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Request {
    pub root: PathBuf,
    pub operation_id: u32,
    pub action: Action,
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum Action {
    Status,
    Diff {
        path: String,
        side: Side,
        revision: String,
    },
    Stage {
        path: String,
        revision: String,
        file_revision: String,
        replace_partial: bool,
    },
    Unstage {
        path: String,
        revision: String,
    },
    Commit {
        paths: Vec<String>,
        message: String,
        revision: String,
    },
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Side {
    Index,
    Worktree,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    repo_root: String,
    branch: Option<String>,
    head_oid: Option<String>,
    revision: String,
    entries: Vec<Entry>,
    outside_staged_count: usize,
    mutation_block: Option<Error>,
    commit_block: Option<Error>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    path: String,
    index: char,
    worktree: char,
    conflict: bool,
    supported: bool,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum Reply {
    Status {
        status: Status,
    },
    Diff {
        text: Option<String>,
        binary: bool,
        file_revision: Option<String>,
    },
    Mutation {
        status: Option<Status>,
        effect: Effect,
        oid: Option<String>,
        branch: Option<String>,
        issue: Option<Error>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Effect {
    Applied,
    NotApplied,
    Unknown,
}

pub(crate) async fn execute(
    root: &Path,
    action: Action,
    token: CancellationToken,
) -> Result<Reply> {
    let executable = process::executable()?;
    run(root, action, token, &executable).await
}

async fn run(
    root: &Path,
    action: Action,
    token: CancellationToken,
    executable: &Path,
) -> Result<Reply> {
    let directory = repository::find(root)?;
    let hooks = tempfile::tempdir().map_err(|_| Error::io())?;
    let duration = Duration::from_secs(if matches!(action, Action::Commit { .. }) {
        60
    } else {
        10
    });
    let git = process::Git::new(executable, &directory, hooks.path(), token, duration);
    repository::Repo::new(root, &git)?.execute(action).await
}
