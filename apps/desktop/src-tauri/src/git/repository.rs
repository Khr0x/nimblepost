use super::{
    Action, Effect, Entry, Error, Reply, Result, Side, Status,
    process::{Git, MAX_BYTES},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, Metadata},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
};

const MAX_ENTRIES: usize = 10_000;
const DIFF_ARGS: &[&str] = &[
    "--no-ext-diff",
    "--no-textconv",
    "--no-color",
    "--no-renames",
];

pub(super) fn find(root: &Path) -> Result<PathBuf> {
    if !root.is_dir()
        || fs::canonicalize(root).ok().as_deref() != Some(root)
        || root
            .components()
            .any(|part| part.as_os_str().eq_ignore_ascii_case(".git"))
    {
        return Err(Error::invalid_path());
    }
    for ancestor in root.ancestors() {
        if let Some(metadata) = metadata(&ancestor.join(".git"))? {
            if !metadata.is_dir() || is_link(&metadata) {
                return Err(Error::unsupported());
            }
            return Ok(ancestor.to_owned());
        }
    }
    Err(Error::new(
        "not-repository",
        "This collection is not in a Git repository.",
    ))
}

pub(super) struct Repo<'a> {
    git: &'a Git<'a>,
    prefix: String,
}

struct Snapshot {
    status: Status,
    index: Option<Vec<u8>>,
}

impl Status {
    fn entry(&self, path: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.path == path)
    }

    fn check(&self, revision: &str) -> Result<()> {
        if self.revision != revision {
            return Err(Error::stale());
        }
        Ok(())
    }

    fn ensure_mutable(&self) -> Result<()> {
        if let Some(error) = &self.mutation_block {
            return Err(Error::new(error.code, error.message));
        }
        Ok(())
    }
}

impl<'a> Repo<'a> {
    pub fn new(root: &Path, git: &'a Git<'a>) -> Result<Self> {
        let prefix = root
            .strip_prefix(git.directory)
            .map_err(|_| Error::invalid_path())?
            .components()
            .map(|part| part.as_os_str().to_str().ok_or_else(Error::unsupported))
            .collect::<Result<Vec<_>>>()?
            .join("/");
        let prefix = if prefix.is_empty() {
            prefix
        } else {
            prefix + "/"
        };
        Ok(Self { git, prefix })
    }

    pub async fn execute(&self, action: Action) -> Result<Reply> {
        let snapshot = self.snapshot().await?;
        match action {
            Action::Status => Ok(Reply::Status {
                status: snapshot.status,
            }),
            Action::Diff {
                path,
                side,
                revision,
            } => {
                snapshot.status.check(&revision)?;
                self.diff(&snapshot.status, &path, side).await
            }
            Action::Stage {
                path,
                revision,
                file_revision,
                replace_partial,
            } => {
                snapshot.status.check(&revision)?;
                snapshot.status.ensure_mutable()?;
                let entry = snapshot.status.entry(&path).ok_or_else(Error::stale)?;
                if entry.worktree == '.' || (entry.index != '.' && !replace_partial) {
                    return Err(Error::new(
                        "invalid-input",
                        "Review the working copy and confirm replacement of partial staging.",
                    ));
                }
                let (relative, mode) = self.path(entry)?;
                if self.file_revision(&relative, mode).await? != file_revision {
                    return Err(Error::stale());
                }
                self.change_index(
                    snapshot,
                    &path,
                    &["add", "--all", "--", &relative],
                    Some(&file_revision),
                )
                .await
            }
            Action::Unstage { path, revision } => {
                snapshot.status.check(&revision)?;
                snapshot.status.ensure_mutable()?;
                let entry = snapshot.status.entry(&path).ok_or_else(Error::stale)?;
                if entry.index == '.' {
                    return Err(Error::stale());
                }
                let (relative, _) = self.path(entry)?;
                let args = if snapshot.status.head_oid.is_some() {
                    vec!["restore", "--staged", "--source=HEAD", "--", &relative]
                } else {
                    vec!["rm", "--cached", "-f", "--", &relative]
                };
                self.change_index(snapshot, &path, &args, None).await
            }
            Action::Commit {
                paths,
                message,
                revision,
            } => {
                snapshot.status.check(&revision)?;
                snapshot.status.ensure_mutable()?;
                self.commit(snapshot, paths, message, &revision).await
            }
        }
    }

    async fn config(&self) -> Result<(Option<Error>, bool)> {
        for directory in [".git", ".git/objects", ".git/refs"] {
            if metadata(&self.git.directory.join(directory))?
                .is_none_or(|item| !item.is_dir() || is_link(&item))
            {
                return Err(Error::unsupported());
            }
        }
        if [
            ".git/commondir",
            ".git/objects/info/alternates",
            ".git/info/grafts",
        ]
        .iter()
        .any(|name| present(&self.git.directory.join(name)))
        {
            return Err(Error::unsupported());
        }
        let config = self
            .git
            .read(&["config", "--null", "--show-scope", "--list", "--includes"])
            .await?;
        let config = utf8(&config)?;
        let mut hooks = self.git.directory.join(".git/hooks");
        let mut signing = false;
        let mut filters = BTreeSet::new();
        let mut records = config.split_terminator('\0');
        while let Some(scope) = records.next() {
            let setting = records.next().ok_or_else(Error::io)?;
            let (key, value) = setting.split_once('\n').unwrap_or((setting, ""));
            let enabled = !matches!(
                value.to_ascii_lowercase().as_str(),
                "false" | "no" | "off" | "0"
            );
            if (matches!(key, "core.bare" | "index.sparse" | "core.splitindex") && enabled)
                || matches!(
                    key,
                    "extensions.partialclone" | "extensions.worktreeconfig" | "core.worktree"
                )
                || (key.starts_with("remote.") && key.ends_with(".promisor") && enabled)
            {
                return Err(Error::unsupported());
            }
            if let Some((driver, setting)) = key.rsplit_once('.')
                && driver.starts_with("filter.")
                && matches!(setting, "clean" | "process" | "smudge" | "required")
            {
                if matches!(scope, "local" | "worktree")
                    && matches!(setting, "clean" | "process")
                    && !value.is_empty()
                {
                    return Err(Error::unsupported());
                }
                filters.insert(driver.to_owned());
            }
            if key == "core.hookspath" {
                hooks = self.git.directory.join(value);
            }
            if key == "commit.gpgsign" {
                signing = enabled;
            }
        }
        let has_commit_hook = [
            "pre-commit",
            "prepare-commit-msg",
            "commit-msg",
            "post-commit",
            "reference-transaction",
        ]
        .iter()
        .any(|name| present(&hooks.join(name)));
        let has_filters = !filters.is_empty();
        self.git.set_filters(filters);
        let block = if signing || has_commit_hook {
            Some(Error::new(
                "unsupported",
                "This repository configures commit signing or hooks. Use Git directly to commit.",
            ))
        } else {
            None
        };
        Ok((block, has_filters))
    }

    async fn index(&self) -> Result<Option<Vec<u8>>> {
        let path = self.git.directory.join(".git/index");
        self.git
            .bound(async {
                tokio::task::spawn_blocking(move || read_index_file(&path))
                    .await
                    .map_err(|_| Error::io())?
            })
            .await
    }

    async fn snapshot(&self) -> Result<Snapshot> {
        let (commit_block, has_filters) = self.config().await?;
        let index = self.index().await?;
        if !self
            .git
            .read(&["rev-parse", "--shared-index-path"])
            .await?
            .trim_ascii()
            .is_empty()
        {
            return Err(Error::unsupported());
        }
        let raw = self
            .git
            .read(&[
                "status",
                "--porcelain=v2",
                "-z",
                "--branch",
                "--no-renames",
                "--untracked-files=all",
                "--ignore-submodules=all",
            ])
            .await?;
        if index != self.index().await? {
            return Err(Error::stale());
        }
        let mut status = parse_status(&raw, &self.prefix)?;
        if has_filters {
            self.mark_filtered_paths(&mut status).await?;
        }
        status.repo_root = self
            .git
            .directory
            .to_str()
            .ok_or_else(Error::unsupported)?
            .to_owned();
        let mut hash = Sha256::new();
        hash.update(status.repo_root.as_bytes());
        hash.update([0]);
        hash.update(self.prefix.as_bytes());
        hash.update([0]);
        hash.update(&raw);
        hash.update([0]);
        if let Some(index) = &index {
            hash.update(index);
        }
        status.revision = format!("{:x}", hash.finalize());
        if status.mutation_block.is_none()
            && [
                "MERGE_HEAD",
                "CHERRY_PICK_HEAD",
                "REVERT_HEAD",
                "rebase-apply",
                "rebase-merge",
                "sequencer",
                "BISECT_START",
            ]
            .iter()
            .any(|name| present(&self.git.directory.join(".git").join(name)))
        {
            status.mutation_block = Some(Error::new(
                "conflict",
                "Finish the Git operation already in progress first.",
            ));
        }
        if status.mutation_block.is_none() && present(&self.git.directory.join(".git/index.lock")) {
            status.mutation_block = Some(Error::new("busy", "Git is holding a repository lock."));
        }
        status.commit_block = commit_block.or_else(|| {
            status.branch.is_none().then(|| {
                Error::new(
                    "unsupported",
                    "Check out a branch in Git before committing.",
                )
            })
        });
        Ok(Snapshot { status, index })
    }

    async fn mark_filtered_paths(&self, status: &mut Status) -> Result<()> {
        let mut input = Vec::new();
        for entry in status.entries.iter().filter(|entry| entry.supported) {
            input.extend_from_slice(self.prefix.as_bytes());
            input.extend_from_slice(entry.path.as_bytes());
            input.push(0);
        }
        if input.is_empty() {
            return Ok(());
        }
        let mut blocked = BTreeSet::new();
        for args in [
            &["check-attr", "-z", "--stdin", "filter"][..],
            &["check-attr", "--cached", "-z", "--stdin", "filter"][..],
        ] {
            let output = self.git.run(args, Some(&input), None).await?.checked()?;
            let mut lines = utf8(&output)?.split_terminator('\0');
            while let Some(path) = lines.next() {
                if lines.next() != Some("filter") {
                    return Err(Error::io());
                }
                let value = lines.next().ok_or_else(Error::io)?;
                if !matches!(value, "unspecified" | "unset") {
                    let relative = path.strip_prefix(&self.prefix).ok_or_else(Error::io)?;
                    blocked.insert(relative.to_owned());
                }
            }
        }
        for entry in &mut status.entries {
            entry.supported &= !blocked.contains(&entry.path);
        }
        Ok(())
    }

    fn path(&self, entry: &Entry) -> Result<(String, Option<u32>)> {
        if !entry.supported || !regular_relative(&entry.path) {
            return Err(Error::invalid_path());
        }
        let relative = format!("{}{}", self.prefix, entry.path);
        let mut absolute = self.git.directory.to_owned();
        let mut parts = relative.split('/').peekable();
        while let Some(part) = parts.next() {
            absolute.push(part);
            let Some(item) = metadata(&absolute)? else {
                return Ok((relative, None));
            };
            if is_link(&item) {
                return Err(Error::invalid_path());
            }
            if parts.peek().is_none() {
                if !item.is_file() {
                    return Err(Error::invalid_path());
                }
                #[cfg(unix)]
                let mode = std::os::unix::fs::PermissionsExt::mode(&item.permissions());
                #[cfg(windows)]
                let mode = u32::from(item.permissions().readonly());
                return Ok((relative, Some(mode)));
            }
            if !item.is_dir() || present(&absolute.join(".git")) {
                return Err(Error::invalid_path());
            }
        }
        Err(Error::invalid_path())
    }

    async fn file_revision(&self, path: &str, mode: Option<u32>) -> Result<String> {
        match mode {
            None => Ok("deleted".to_owned()),
            Some(mode) => {
                let output = self
                    .git
                    .read(&["hash-object", "--no-filters", "--", path])
                    .await?;
                Ok(format!("{}:{mode}", oid(&output)?))
            }
        }
    }

    async fn diff(&self, status: &Status, path: &str, side: Side) -> Result<Reply> {
        let entry = status.entry(path).ok_or_else(Error::invalid_path)?;
        let (relative, mode) = self.path(entry)?;
        let file_revision = match side {
            Side::Worktree => Some(self.file_revision(&relative, mode).await?),
            Side::Index => None,
        };
        let untracked = matches!(side, Side::Worktree) && entry.worktree == '?';
        let mut args = vec!["diff"];
        args.extend_from_slice(DIFF_ARGS);
        if matches!(side, Side::Index) {
            args.push("--cached");
        } else if untracked {
            args.push("--no-index");
        }
        args.push("--");
        if untracked {
            args.push("/dev/null");
        }
        args.push(&relative);
        let output = self.git.run(&args, None, None).await?;
        let bytes = if untracked && output.code == Some(1) {
            output.bytes
        } else {
            output.checked()?
        };
        let fresh = self.snapshot().await?;
        fresh.status.check(&status.revision)?;
        if let Some(expected) = &file_revision {
            let (relative, mode) = self.path(entry)?;
            if self.file_revision(&relative, mode).await? != *expected {
                return Err(Error::stale());
            }
        }
        let text = String::from_utf8_lossy(&bytes);
        let binary = text
            .lines()
            .any(|line| line.starts_with("Binary files ") && line.ends_with(" differ"));
        Ok(Reply::Diff {
            text: (!binary).then(|| text.into_owned()),
            binary,
            file_revision,
        })
    }

    async fn change_index(
        &self,
        before: Snapshot,
        path: &str,
        args: &[&str],
        file_revision: Option<&str>,
    ) -> Result<Reply> {
        let applied = async {
            let temporary = tempfile::tempdir().map_err(|_| Error::io())?;
            let index = temporary.path().join("index");
            if let Some(original) = &before.index {
                tokio::fs::write(&index, original)
                    .await
                    .map_err(|_| Error::io())?;
            }
            self.git.run(args, None, Some(&index)).await?.checked()?;
            let fresh = self.snapshot().await?;
            fresh.status.check(&before.status.revision)?;
            fresh.status.ensure_mutable()?;
            if let Some(expected) = file_revision {
                let entry = fresh.status.entry(path).ok_or_else(Error::stale)?;
                let (relative, mode) = self.path(entry)?;
                if self.file_revision(&relative, mode).await? != expected {
                    return Err(Error::stale());
                }
            }
            let new_index = read_index_file(&index)?.ok_or_else(Error::io)?;
            self.git.bound(std::future::ready(Ok(()))).await?;
            self.publish_index(before.index.as_deref(), &new_index)
        }
        .await;
        let git = self.git.reconcile();
        let status = Repo {
            git: &git,
            prefix: self.prefix.clone(),
        }
        .snapshot()
        .await
        .ok();
        Ok(Reply::Mutation {
            status: status.map(|snapshot| snapshot.status),
            effect: if applied.is_ok() {
                Effect::Applied
            } else {
                Effect::NotApplied
            },
            oid: None,
            branch: None,
            issue: applied.err(),
        })
    }

    fn publish_index(&self, original: Option<&[u8]>, replacement: &[u8]) -> Result<()> {
        let index = self.git.directory.join(".git/index");
        let lock = index.with_extension("lock");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
            .map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    Error::new("busy", "Git is holding a repository lock.")
                } else {
                    Error::io()
                }
            })?;
        // The observed index must still match while we hold Git's index.lock.
        let prepared = (|| {
            if read_index_file(&index)?.as_deref() != original {
                return Err(Error::stale());
            }
            if let Some(item) = metadata(&index)? {
                file.set_permissions(item.permissions())
                    .map_err(|_| Error::io())?;
            }
            file.write_all(replacement)
                .and_then(|()| file.sync_all())
                .map_err(|_| Error::io())
        })();
        drop(file);
        let published = prepared.and_then(|()| fs::rename(&lock, index).map_err(|_| Error::io()));
        if published.is_err() {
            let _ = fs::remove_file(&lock);
        }
        published
    }

    async fn commit(
        &self,
        before: Snapshot,
        paths: Vec<String>,
        message: String,
        revision: &str,
    ) -> Result<Reply> {
        let status = &before.status;
        if let Some(error) = &status.commit_block {
            return Err(Error::new(error.code, error.message));
        }
        let branch = status.branch.as_deref().ok_or_else(Error::stale)?;
        let symbolic = self.git.read(&["symbolic-ref", "-q", "HEAD"]).await?;
        let reference = format!("refs/heads/{branch}");
        if utf8(symbolic.trim_ascii_end())? != reference {
            return Err(Error::stale());
        }
        let selected: BTreeSet<_> = paths.iter().map(String::as_str).collect();
        if selected.is_empty()
            || selected.len() != paths.len()
            || message.trim().is_empty()
            || message.contains('\0')
        {
            return Err(Error::new(
                "invalid-input",
                "Select unique prepared files and enter a commit message.",
            ));
        }
        let staged: BTreeSet<_> = status
            .entries
            .iter()
            .filter(|entry| entry.index != '.')
            .map(|entry| entry.path.as_str())
            .collect();
        if status.outside_staged_count != 0 || staged != selected {
            return Err(Error::new(
                "selection-mismatch",
                "Select all prepared files in this repository. Prepared files outside this collection must be handled in Git.",
            ));
        }
        let mut expected = BTreeSet::new();
        for path in selected {
            let entry = status.entry(path).ok_or_else(Error::stale)?;
            expected.insert(self.path(entry)?.0);
        }
        for ident in ["GIT_AUTHOR_IDENT", "GIT_COMMITTER_IDENT"] {
            let identity = self.git.run(&["var", ident], None, None).await?;
            let valid = identity.success()
                && utf8(&identity.bytes)?
                    .rsplit_once(" <")
                    .and_then(|(name, tail)| tail.split_once('>').map(|(email, _)| (name, email)))
                    .is_some_and(|(name, email)| {
                        !name.trim().is_empty() && !email.trim().is_empty()
                    });
            if !valid {
                return Err(Error::new(
                    "identity",
                    "Configure a Git name and email before committing.",
                ));
            }
        }
        let temporary = tempfile::tempdir().map_err(|_| Error::io())?;
        let index = temporary.path().join("index");
        tokio::fs::write(&index, before.index.as_ref().ok_or_else(Error::stale)?)
            .await
            .map_err(|_| Error::io())?;
        let tree = self
            .git
            .run(&["write-tree"], None, Some(&index))
            .await?
            .checked()?;
        let tree = oid(&tree)?;
        let delta = if let Some(head) = &status.head_oid {
            self.git
                .read(&[
                    "diff-tree",
                    "--no-commit-id",
                    "-r",
                    "-z",
                    "--name-only",
                    "--no-renames",
                    head,
                    tree,
                ])
                .await?
        } else {
            self.git
                .read(&["ls-tree", "-r", "-z", "--name-only", tree])
                .await?
        };
        let actual = utf8(&delta)?
            .split_terminator('\0')
            .collect::<BTreeSet<_>>();
        if !actual.into_iter().eq(expected.iter().map(String::as_str)) {
            return Err(Error::stale());
        }
        let mut args = vec!["commit-tree", tree, "-F", "-"];
        if let Some(head) = &status.head_oid {
            args.extend(["-p", head]);
        }
        let candidate = self
            .git
            .run(&args, Some(message.as_bytes()), None)
            .await?
            .checked()?;
        let candidate = oid(&candidate)?;
        let fresh = self.snapshot().await?;
        fresh.status.check(revision)?;
        fresh.status.ensure_mutable()?;
        if let Some(error) = fresh.status.commit_block {
            return Err(error);
        }
        if self
            .git
            .read(&["symbolic-ref", "-q", "HEAD"])
            .await?
            .trim_ascii_end()
            != reference.as_bytes()
        {
            return Err(Error::stale());
        }
        let zero = "0".repeat(candidate.len());
        let previous = status.head_oid.as_deref().unwrap_or(&zero);
        // The named ref protects another branch if an external Git switches HEAD here.
        let applied = self
            .git
            .run(
                &[
                    "update-ref",
                    "--no-deref",
                    "-m",
                    "commit (NimblePost)",
                    &reference,
                    candidate,
                    previous,
                ],
                None,
                None,
            )
            .await
            .and_then(|output| output.checked());
        let git = self.git.reconcile();
        let effect = if applied.is_ok() {
            Effect::Applied
        } else {
            match git
                .run(
                    &[
                        "rev-parse",
                        "--verify",
                        "--quiet",
                        "--end-of-options",
                        &reference,
                    ],
                    None,
                    None,
                )
                .await
            {
                Ok(output)
                    if output.success() && output.bytes.trim_ascii() == candidate.as_bytes() =>
                {
                    Effect::Applied
                }
                Ok(output)
                    if output.success() && output.bytes.trim_ascii() == previous.as_bytes() =>
                {
                    Effect::NotApplied
                }
                Ok(output) if output.code == Some(1) && previous == zero => Effect::NotApplied,
                _ => Effect::Unknown,
            }
        };
        let status = Repo {
            git: &git,
            prefix: self.prefix.clone(),
        }
        .snapshot()
        .await
        .ok()
        .map(|snapshot| snapshot.status);
        Ok(Reply::Mutation {
            status,
            effect,
            oid: (effect == Effect::Applied).then(|| candidate.to_owned()),
            branch: Some(branch.to_owned()),
            issue: applied.err(),
        })
    }
}

fn parse_status(raw: &[u8], prefix: &str) -> Result<Status> {
    let mut status = Status {
        repo_root: String::new(),
        branch: None,
        head_oid: None,
        revision: String::new(),
        entries: Vec::new(),
        outside_staged_count: 0,
        mutation_block: None,
        commit_block: None,
    };
    let mut count = 0;
    for record in utf8(raw)?.split_terminator('\0') {
        if let Some(value) = record.strip_prefix("# branch.head ") {
            status.branch = (value != "(detached)").then(|| value.to_owned());
        } else if let Some(value) = record.strip_prefix("# branch.oid ") {
            if value != "(initial)" {
                status.head_oid = Some(oid(value.as_bytes())?.to_owned());
            }
        } else if !record.starts_with("# ") {
            count += 1;
            if count > MAX_ENTRIES {
                return Err(Error::limit());
            }
            let (index, worktree, path, conflict, modes) = if let Some(path) =
                record.strip_prefix("? ")
            {
                ('.', '?', path, false, true)
            } else {
                let conflict = record.starts_with("u ");
                let columns: Vec<_> = record.splitn(if conflict { 11 } else { 9 }, ' ').collect();
                if columns.len() != if conflict { 11 } else { 9 }
                    || (!conflict && columns[0] != "1")
                    || columns[1].len() != 2
                    || !columns[1].is_ascii()
                {
                    return Err(Error::io());
                }
                let code = columns[1].as_bytes();
                let modes = &columns[3..if conflict { 7 } else { 6 }];
                let regular = modes
                    .iter()
                    .all(|mode| matches!(*mode, "000000" | "100644" | "100755"));
                (
                    char::from(code[0]),
                    char::from(code[1]),
                    columns[columns.len() - 1],
                    conflict,
                    regular,
                )
            };
            if conflict {
                status.mutation_block = Some(Error::new(
                    "conflict",
                    "Resolve the Git conflicts before changing the index.",
                ));
            }
            if let Some(path) = path.strip_prefix(prefix) {
                status.entries.push(Entry {
                    path: path.to_owned(),
                    index,
                    worktree,
                    conflict,
                    supported: modes && regular_relative(path),
                });
            } else if index != '.' {
                status.outside_staged_count += 1;
            }
        }
    }
    Ok(status)
}

fn regular_relative(path: &str) -> bool {
    !path.is_empty()
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.eq_ignore_ascii_case(".git")
                && !part.contains('\0')
                && (!cfg!(windows)
                    || (!part.contains(['\\', ':'])
                        && !part.ends_with(['.', ' '])
                        && !part.eq_ignore_ascii_case("git~1")))
                && Path::new(part)
                    .components()
                    .eq([Component::Normal(part.as_ref())])
        })
}

fn metadata(path: &Path) -> Result<Option<Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(item) => Ok(Some(item)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(Error::io()),
    }
}

fn read_index_file(path: &Path) -> Result<Option<Vec<u8>>> {
    let Some(item) = metadata(path)? else {
        return Ok(None);
    };
    if !item.is_file() || is_link(&item) {
        return Err(Error::unsupported());
    }
    if item.len() > MAX_BYTES as u64 {
        return Err(Error::limit());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| Error::io())?
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::io())?;
    if bytes.len() > MAX_BYTES {
        return Err(Error::limit());
    }
    Ok(Some(bytes))
}

fn is_link(item: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        item.file_attributes() & 0x400 != 0 // FILE_ATTRIBUTE_REPARSE_POINT, including junctions
    }
    #[cfg(not(windows))]
    {
        item.file_type().is_symlink()
    }
}

fn present(path: &Path) -> bool {
    !matches!(path.symlink_metadata(), Err(error) if error.kind() == io::ErrorKind::NotFound)
}

fn utf8(bytes: &[u8]) -> Result<&str> {
    std::str::from_utf8(bytes).map_err(|_| Error::unsupported())
}

fn oid(bytes: &[u8]) -> Result<&str> {
    let bytes = bytes.trim_ascii();
    if matches!(bytes.len(), 40 | 64) && bytes.iter().all(u8::is_ascii_hexdigit) {
        utf8(bytes)
    } else {
        Err(Error::io())
    }
}
