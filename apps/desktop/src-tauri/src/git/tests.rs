use super::*;
use std::{fs, process::Command};
use tempfile::TempDir;

struct Fixture {
    _temporary: TempDir,
    root: PathBuf,
    collection: PathBuf,
    executable: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let collection = root.join("collection");
        fs::create_dir(&collection).unwrap();
        let this = Self {
            _temporary: temporary,
            root,
            collection,
            executable: process::executable().unwrap(),
        };
        this.git(&["init", "-q", "-b", "main"]);
        this.git(&["config", "user.name", "Git fixture"]);
        this.git(&["config", "user.email", "fixture@example.invalid"]);
        this.git(&["config", "commit.gpgsign", "false"]);
        this
    }

    fn git(&self, args: &[&str]) -> Vec<u8> {
        let output = Command::new(&self.executable)
            .current_dir(&self.root)
            .args([
                "-c",
                "core.hooksPath=.git/fixture-no-hooks",
                "-c",
                "core.fsmonitor=false",
                "--literal-pathspecs",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fixture Git: {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }

    fn write(&self, name: &str, bytes: &[u8]) {
        fs::write(self.collection.join(name), bytes).unwrap();
    }

    fn index(&self) -> Vec<u8> {
        fs::read(self.root.join(".git/index")).unwrap()
    }

    async fn call(&self, action: Action) -> Result<Reply> {
        run(
            &self.collection,
            action,
            CancellationToken::new(),
            &self.executable,
        )
        .await
    }

    async fn status(&self) -> Status {
        let Reply::Status { status } = self.call(Action::Status).await.unwrap() else {
            panic!("expected status")
        };
        status
    }

    async fn diff(&self, status: &Status, path: &str, side: Side) -> Result<Reply> {
        self.call(Action::Diff {
            path: path.into(),
            side,
            revision: status.revision.clone(),
        })
        .await
    }

    async fn prepare(&self, path: &str, replace_partial: bool) -> Result<Reply> {
        let status = self.status().await;
        let Reply::Diff {
            file_revision: Some(file_revision),
            ..
        } = self.diff(&status, path, Side::Worktree).await?
        else {
            panic!("expected a working tree revision");
        };
        self.call(Action::Stage {
            path: path.into(),
            revision: status.revision,
            file_revision,
            replace_partial,
        })
        .await
    }

    async fn unstage(&self, status: Status, path: &str) -> Status {
        mutation(
            self.call(Action::Unstage {
                paths: vec![path.into()],
                revision: status.revision,
            })
            .await
            .unwrap(),
        )
    }

    async fn commit(&self, status: &Status, paths: &[&str]) -> Result<Reply> {
        self.call(commit_action(status, paths)).await
    }
}

fn commit_action(status: &Status, paths: &[&str]) -> Action {
    Action::Commit {
        paths: paths.iter().map(|path| (*path).to_owned()).collect(),
        revision: status.revision.clone(),
        message: "Execute fixture revision\n\nA literal message.".into(),
    }
}

fn mutation(reply: Reply) -> Status {
    let Reply::Mutation {
        status: Some(status),
        effect: Effect::Applied,
        issue: None,
        ..
    } = reply
    else {
        panic!("expected applied mutation: {reply:?}")
    };
    status
}

fn entry<'a>(status: &'a Status, name: &str) -> &'a Entry {
    status
        .entries
        .iter()
        .find(|entry| entry.path == name)
        .expect("file is listed")
}

#[tokio::test]
async fn status_and_diffs_cover_unborn_text_binary_and_deletion() {
    let fixture = Fixture::new();
    fixture.write("one.yml", b"example text\n");
    fixture.write("binary.dat", b"\0\x01\x02");
    fixture.write(".gitignore", b"ignored.txt\n");
    fixture.write("ignored.txt", b"ignored\n");
    let status = fixture.status().await;
    assert_eq!(status.branch.as_deref(), Some("main"));
    assert!(status.head_oid.is_none());
    assert_eq!(status.repo_root, fixture.root.to_str().unwrap());
    assert_eq!(entry(&status, "one.yml").worktree, '?');
    assert!(
        !status
            .entries
            .iter()
            .any(|entry| entry.path == "ignored.txt")
    );

    let Reply::Diff {
        text: Some(text),
        file_revision: Some(_),
        ..
    } = fixture
        .diff(&status, "one.yml", Side::Worktree)
        .await
        .unwrap()
    else {
        panic!("expected text diff")
    };
    assert!(text.contains("+example text\n"));
    assert!(matches!(
        fixture
            .diff(&status, "binary.dat", Side::Worktree)
            .await
            .unwrap(),
        Reply::Diff {
            binary: true,
            text: None,
            ..
        }
    ));
    assert!(!fixture.root.join(".git/index").exists());

    let status = mutation(fixture.prepare("one.yml", false).await.unwrap());
    let original = fixture.index();
    let Reply::Diff {
        text: Some(text),
        file_revision: None,
        ..
    } = fixture.diff(&status, "one.yml", Side::Index).await.unwrap()
    else {
        panic!("expected staged diff")
    };
    assert!(text.contains("+example text\n"));
    assert_eq!(fixture.index(), original);
    mutation(fixture.commit(&status, &["one.yml"]).await.unwrap());

    fs::remove_file(fixture.collection.join("one.yml")).unwrap();
    let status = fixture.status().await;
    let Reply::Diff {
        text: Some(text), ..
    } = fixture
        .diff(&status, "one.yml", Side::Worktree)
        .await
        .unwrap()
    else {
        panic!("expected removal diff")
    };
    assert!(text.contains("-example text\n"));
    assert_eq!(fixture.index(), original);
    assert!(!fixture.collection.join("one.yml").exists());
}

#[tokio::test]
async fn stage_paths_adds_the_exact_batch_and_keeps_existing_index_versions() {
    let fixture = Fixture::new();
    for name in ["modified.yml", "deleted.yml", "partial.yml"] {
        fixture.write(name, b"original\n");
    }
    fixture.git(&["add", "--", "collection"]);
    fixture.git(&["commit", "-qm", "initial"]);
    fixture.write("modified.yml", b"new content\n");
    fs::remove_file(fixture.collection.join("deleted.yml")).unwrap();
    fixture.write("partial.yml", b"index version\n");
    fixture.git(&["add", "--", "collection/partial.yml"]);
    fixture.write("partial.yml", b"worktree version\n");
    fixture.write("unselected.yml", b"unselected\n");
    fs::write(fixture.root.join("outside.yml"), b"outside\n").unwrap();
    fixture.git(&["add", "--", "outside.yml"]);
    let name = if cfg!(windows) {
        "-with space-é.yml"
    } else {
        "-with space-é\"\\\tline\nbreak.yml"
    };
    fixture.write(name, b"added in the batch\n");
    #[cfg(unix)]
    std::os::unix::fs::symlink("modified.yml", fixture.collection.join("link.yml")).unwrap();

    let status = fixture.status().await;
    #[cfg(unix)]
    assert!(
        entry(&status, "link.yml")
            .blocked_reason
            .unwrap()
            .contains("symbolic link")
    );
    let original = fixture.index();
    let rejected = fixture
        .call(Action::StagePaths {
            paths: vec![name.into(), "partial.yml".into()],
            revision: status.revision.clone(),
        })
        .await
        .unwrap_err();
    assert_eq!(rejected.code, "stale-state");
    assert_eq!(fixture.index(), original);

    let reply = fixture
        .call(Action::StagePaths {
            paths: [name, "modified.yml", "deleted.yml"]
                .map(str::to_owned)
                .into(),
            revision: status.revision,
        })
        .await
        .unwrap();
    assert!(matches!(
        reply,
        Reply::Mutation {
            selection_stable: true,
            ..
        }
    ));
    let status = mutation(reply);
    assert_eq!(entry(&status, name).index, 'A');
    assert_eq!(entry(&status, "modified.yml").index, 'M');
    assert_eq!(entry(&status, "deleted.yml").index, 'D');
    assert_eq!(entry(&status, "unselected.yml").index, '.');
    assert_eq!(entry(&status, "partial.yml").worktree, 'M');
    assert_eq!(status.outside_staged_count, 1);
    assert_eq!(
        fixture.git(&["show", ":collection/partial.yml"]),
        b"index version\n"
    );
    assert_eq!(
        fixture.git(&["show", &format!(":collection/{name}")]),
        b"added in the batch\n"
    );
    assert_eq!(fixture.git(&["show", ":outside.yml"]), b"outside\n");

    let status = mutation(
        fixture
            .call(Action::Unstage {
                paths: [name, "modified.yml", "deleted.yml", "partial.yml"]
                    .map(str::to_owned)
                    .into(),
                revision: status.revision,
            })
            .await
            .unwrap(),
    );
    for (path, worktree) in [
        (name, '?'),
        ("modified.yml", 'M'),
        ("deleted.yml", 'D'),
        ("partial.yml", 'M'),
    ] {
        assert_eq!(
            (entry(&status, path).index, entry(&status, path).worktree),
            ('.', worktree)
        );
    }
    assert_eq!(
        fs::read(fixture.collection.join(name)).unwrap(),
        b"added in the batch\n"
    );
    assert_eq!(
        fs::read(fixture.collection.join("modified.yml")).unwrap(),
        b"new content\n"
    );
    assert_eq!(
        fs::read(fixture.collection.join("partial.yml")).unwrap(),
        b"worktree version\n"
    );
    assert!(!fixture.collection.join("deleted.yml").exists());
    assert_eq!(
        fixture.git(&["diff", "--cached", "--name-only"]),
        b"outside.yml\n"
    );
    assert_eq!(fixture.git(&["show", ":outside.yml"]), b"outside\n");
    #[cfg(unix)]
    {
        fixture.git(&["add", "--", "collection/link.yml"]);
        let status = fixture.status().await;
        let link = entry(&status, "link.yml");
        assert_eq!(link.index, 'A');
        assert!(link.blocked_reason.unwrap().contains("symbolic link"));
    }
}

#[tokio::test]
async fn unstage_paths_before_the_first_commit_keeps_every_file_on_disk() {
    let fixture = Fixture::new();
    fixture.write("one", b"new one\n");
    fixture.write("two", b"new two\n");
    fixture.git(&["add", "--", "collection"]);
    let status = fixture.status().await;
    let status = mutation(
        fixture
            .call(Action::Unstage {
                paths: vec!["one".into(), "two".into()],
                revision: status.revision,
            })
            .await
            .unwrap(),
    );
    assert!(status.head_oid.is_none());
    assert!(
        status
            .entries
            .iter()
            .all(|entry| entry.index == '.' && entry.worktree == '?')
    );
    assert!(fixture.git(&["ls-files"]).is_empty());
    assert_eq!(
        fs::read(fixture.collection.join("one")).unwrap(),
        b"new one\n"
    );
    assert_eq!(
        fs::read(fixture.collection.join("two")).unwrap(),
        b"new two\n"
    );
}

#[tokio::test]
async fn unstage_rejects_restoring_a_file_over_an_unselected_staged_child() {
    let fixture = Fixture::new();
    fixture.write("node", b"original file\n");
    fixture.git(&["add", "--", "collection"]);
    fixture.git(&["commit", "-qm", "initial"]);
    let node = fixture.collection.join("node");
    fs::remove_file(&node).unwrap();
    fs::create_dir(&node).unwrap();
    fixture.write("node/child", b"staged child\n");
    fixture.git(&["add", "-A", "--", "collection"]);
    fs::remove_file(node.join("child")).unwrap();
    fs::remove_dir(&node).unwrap();
    let status = fixture.status().await;
    let original = fixture.index();

    let reply = fixture
        .call(Action::Unstage {
            paths: vec!["node".into()],
            revision: status.revision,
        })
        .await
        .unwrap();
    assert!(matches!(
        reply,
        Reply::Mutation {
            effect: Effect::NotApplied,
            ..
        }
    ));
    assert_eq!(fixture.index(), original);
    assert_eq!(
        fixture.git(&["show", ":collection/node/child"]),
        b"staged child\n"
    );
    assert!(!node.exists());
}

#[tokio::test]
async fn stage_unstage_and_commit_preserve_partial_and_unselected_files() {
    let fixture = Fixture::new();
    let name = if cfg!(windows) {
        "-with space-é.yml"
    } else {
        "-with space-é\tline\nbreak.yml"
    };
    fixture.write(name, b"first\n");
    fixture.write("unselected.yml", b"untouched\n");
    let status = mutation(fixture.prepare(name, false).await.unwrap());
    let status = fixture.unstage(status, name).await;
    assert_eq!(entry(&status, name).worktree, '?');
    assert_eq!(fs::read(fixture.collection.join(name)).unwrap(), b"first\n");

    mutation(fixture.prepare(name, false).await.unwrap());
    fixture.write(name, b"draft on disk\n");
    assert_eq!(
        fixture.prepare(name, false).await.unwrap_err().code,
        "invalid-input"
    );
    let status = fixture.status().await;
    assert_eq!(entry(&status, name).index, 'A');
    assert_eq!(entry(&status, name).worktree, 'M');
    let index = fixture.index();
    assert_eq!(
        fixture
            .commit(&status, &[name, name])
            .await
            .unwrap_err()
            .code,
        "invalid-input"
    );
    let reply = fixture.commit(&status, &[name]).await.unwrap();
    let Reply::Mutation {
        effect: Effect::Applied,
        oid: Some(first),
        status: Some(status),
        ..
    } = reply
    else {
        panic!("commit failed: {reply:?}")
    };
    assert_eq!(
        fixture.git(&["rev-parse", "HEAD"]).trim_ascii(),
        first.as_bytes()
    );
    assert_eq!(
        fixture.git(&["ls-tree", "-r", "-z", "--name-only", "HEAD"]),
        format!("collection/{name}\0").as_bytes()
    );
    assert_eq!(
        fixture.git(&["show", &format!("HEAD:collection/{name}")]),
        b"first\n"
    );
    assert_eq!(fixture.index(), index);
    assert_eq!(
        fs::read(fixture.collection.join(name)).unwrap(),
        b"draft on disk\n"
    );
    assert_eq!(entry(&status, "unselected.yml").worktree, '?');

    mutation(fixture.prepare(name, false).await.unwrap());
    fixture.write(name, b"replacement\n");
    let status = mutation(fixture.prepare(name, true).await.unwrap());
    assert_eq!(entry(&status, name).worktree, '.');
    let status = fixture.unstage(status, name).await;
    assert_eq!(entry(&status, name).index, '.');
    assert_eq!(
        fs::read(fixture.collection.join(name)).unwrap(),
        b"replacement\n"
    );
    let status = mutation(fixture.prepare(name, false).await.unwrap());
    mutation(fixture.commit(&status, &[name]).await.unwrap());
    assert_eq!(
        fixture.git(&["rev-parse", "HEAD^"]).trim_ascii(),
        first.as_bytes()
    );
    assert_eq!(
        fixture.git(&["show", &format!("HEAD:collection/{name}")]),
        b"replacement\n"
    );

    fs::remove_file(fixture.collection.join(name)).unwrap();
    let status = mutation(fixture.prepare(name, false).await.unwrap());
    assert_eq!(entry(&status, name).index, 'D');
    let status = fixture.unstage(status, name).await;
    assert_eq!(entry(&status, name).worktree, 'D');
    assert!(!fixture.collection.join(name).exists());
}

#[tokio::test]
async fn rejects_stale_scope_and_omitted_staged_files_without_mutating() {
    let fixture = Fixture::new();
    fixture.write("one", b"initial\n");
    fixture.write("two", b"two\n");
    let status = fixture.status().await;
    let Reply::Diff {
        file_revision: Some(file_revision),
        ..
    } = fixture.diff(&status, "one", Side::Worktree).await.unwrap()
    else {
        panic!("expected file revision")
    };
    fixture.write("one", b"changed without changing status letters\n");
    assert_eq!(
        fixture
            .call(Action::Stage {
                path: "one".into(),
                revision: status.revision,
                file_revision,
                replace_partial: false
            })
            .await
            .unwrap_err()
            .code,
        "stale-state"
    );
    assert!(!fixture.root.join(".git/index").exists());
    let old = mutation(fixture.prepare("one", false).await.unwrap());
    mutation(fixture.prepare("two", false).await.unwrap());
    assert_eq!(
        fixture.commit(&old, &["one"]).await.unwrap_err().code,
        "stale-state"
    );
    let status = fixture.status().await;
    assert_eq!(
        fixture.commit(&status, &["one"]).await.unwrap_err().code,
        "selection-mismatch"
    );

    let status = fixture.unstage(status, "two").await;
    assert_eq!(entry(&status, "one").index, 'A');
    fs::write(fixture.root.join("sibling-private.yml"), b"sibling\n").unwrap();
    fixture.git(&["add", "--", "sibling-private.yml"]);
    let status = fixture.status().await;
    let index = fixture.index();
    assert_eq!(status.outside_staged_count, 1);
    assert!(
        !serde_json::to_string(&status)
            .unwrap()
            .contains("sibling-private")
    );
    assert_eq!(
        fixture.commit(&status, &["one"]).await.unwrap_err().code,
        "selection-mismatch"
    );
    for path in [
        "../sibling-private.yml",
        ".git/config",
        "one/../one",
        "/tmp/one",
        "sibling-private.yml",
    ] {
        assert_eq!(
            fixture
                .diff(&status, path, Side::Index)
                .await
                .unwrap_err()
                .code,
            "invalid-path"
        );
    }
    assert_eq!(fixture.index(), index);
}

#[tokio::test]
async fn rejects_unavailable_detached_conflicted_locked_and_missing_identity() {
    let fixture = Fixture::new();
    let external = tempfile::tempdir().unwrap();
    let path = external.path().canonicalize().unwrap();
    assert_eq!(
        run(
            &path,
            Action::Status,
            CancellationToken::new(),
            &fixture.executable
        )
        .await
        .unwrap_err()
        .code,
        "not-repository"
    );
    assert_eq!(
        run(
            &fixture.collection,
            Action::Status,
            CancellationToken::new(),
            &path.join("absent-git")
        )
        .await
        .unwrap_err()
        .code,
        "executable-unavailable"
    );

    fixture.write("one", b"first\n");
    let status = mutation(fixture.prepare("one", false).await.unwrap());
    fixture.git(&["config", "user.email", ""]);
    assert_eq!(
        fixture.commit(&status, &["one"]).await.unwrap_err().code,
        "identity"
    );
    fixture.git(&["config", "user.email", "fixture@example.invalid"]);

    fs::write(fixture.root.join(".git/index.lock"), b"held").unwrap();
    let index = fixture.index();
    assert_eq!(
        fixture.commit(&status, &["one"]).await.unwrap_err().code,
        "busy"
    );
    assert_eq!(
        fs::read(fixture.root.join(".git/index.lock")).unwrap(),
        b"held"
    );
    fs::remove_file(fixture.root.join(".git/index.lock")).unwrap();
    assert_eq!(fixture.index(), index);

    mutation(fixture.commit(&status, &["one"]).await.unwrap());
    fixture.git(&["checkout", "--detach", "-q"]);
    fixture.write("one", b"detached version\n");
    let status = mutation(fixture.prepare("one", false).await.unwrap());
    assert_eq!(
        fixture.commit(&status, &["one"]).await.unwrap_err().code,
        "unsupported"
    );

    fixture.git(&["restore", "--staged", "--worktree", "--", "collection/one"]);
    fixture.git(&["switch", "main"]);
    fixture.git(&["switch", "-c", "second"]);
    fixture.write("one", b"second version\n");
    let status = mutation(fixture.prepare("one", false).await.unwrap());
    mutation(fixture.commit(&status, &["one"]).await.unwrap());
    fixture.git(&["switch", "main"]);
    fixture.write("one", b"main version\n");
    let status = mutation(fixture.prepare("one", false).await.unwrap());
    mutation(fixture.commit(&status, &["one"]).await.unwrap());
    let merge = Command::new(&fixture.executable)
        .current_dir(&fixture.root)
        .args(["merge", "--no-edit", "second"])
        .output()
        .unwrap();
    assert!(!merge.status.success());
    let conflicted = fixture.status().await;
    assert!(entry(&conflicted, "one").conflict);
    assert_eq!(
        fixture
            .commit(&conflicted, &["one"])
            .await
            .unwrap_err()
            .code,
        "conflict"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn repository_helpers_and_symlinks_are_never_followed() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let fixture = Fixture::new();
    fixture.write("one", b"text\n");
    fixture.write("outside", b"file\n");
    symlink(
        fixture.collection.join("outside"),
        fixture.collection.join("link"),
    )
    .unwrap();
    let hook = fixture.root.join(".git/hooks/pre-commit");
    fs::write(&hook, b"#!/bin/sh\nprintf hook > SHOULD_NOT_EXIST\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    fixture.git(&[
        "config",
        "core.fsmonitor",
        "printf fsmonitor > SHOULD_NOT_EXIST",
    ]);
    fixture.git(&[
        "config",
        "core.sshCommand",
        "printf helper > SHOULD_NOT_EXIST",
    ]);
    fixture.git(&["config", "diff.external", "printf diff > SHOULD_NOT_EXIST"]);
    fixture.git(&[
        "config",
        "diff.fixture.textconv",
        "printf textconv > SHOULD_NOT_EXIST",
    ]);
    fixture.write(".gitattributes", b"one diff=fixture\n");
    let status = fixture.status().await;
    assert_eq!(
        fixture
            .diff(&status, "link", Side::Worktree)
            .await
            .unwrap_err()
            .code,
        "invalid-path"
    );
    let status = mutation(fixture.prepare("one", false).await.unwrap());
    assert!(
        matches!(fixture.diff(&status, "one", Side::Index).await.unwrap(), Reply::Diff { text: Some(text), .. } if text.contains("+text"))
    );
    assert_eq!(
        fixture.commit(&status, &["one"]).await.unwrap_err().code,
        "unsupported"
    );

    fs::remove_file(hook).unwrap();
    fixture.git(&[
        "config",
        "filter.fixture.clean",
        "printf filter > SHOULD_NOT_EXIST",
    ]);
    fixture.write(".gitattributes", b"one filter=fixture\n");
    assert_eq!(
        fixture.call(Action::Status).await.unwrap_err().code,
        "unsupported"
    );
    assert!(!fixture.root.join("SHOULD_NOT_EXIST").exists());

    let metadata = tempfile::tempdir().unwrap();
    let linked = metadata.path().canonicalize().unwrap();
    fs::write(linked.join(".git"), b"gitdir: elsewhere\n").unwrap();
    assert_eq!(
        run(
            &linked,
            Action::Status,
            CancellationToken::new(),
            &fixture.executable
        )
        .await
        .unwrap_err()
        .code,
        "unsupported"
    );
}

#[cfg(unix)]
fn wrapper(fixture: &Fixture, script: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = fixture.root.join(".git/fixture-git");
    fs::write(
        &path,
        format!(
            "#!/bin/sh\nreal='{}'\n{script}\nexec \"$real\" \"$@\"\n",
            fixture.executable.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[cfg(unix)]
#[tokio::test]
async fn cancellation_reaps_process_and_reconciles_before_and_after_ref_update() {
    let fixture = Fixture::new();
    fixture.write("one", b"to commit\n");
    let status = mutation(fixture.prepare("one", false).await.unwrap());
    let index = fixture.index();
    for publish in [false, true] {
        let marker = fixture.root.join(".git/at-publish");
        let run_real = if publish {
            "\"$real\" \"$@\" || exit $?"
        } else {
            ":"
        };
        let executable = wrapper(
            &fixture,
            &format!(
                r#"for arg in "$@"; do
                if [ "$arg" = update-ref ]; then
                    {run_real}
                    printf '%s' "$$" > .git/at-publish
                    exec /bin/sleep 25
                fi
            done"#
            ),
        );
        let token = CancellationToken::new();
        let cancel = token.clone();
        let stopped = marker.clone();
        let (mutated, pid) = tokio::join!(
            run(
                &fixture.collection,
                commit_action(&status, &["one"]),
                token,
                &executable
            ),
            async {
                let pid = tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        if let Ok(pid) = fs::read_to_string(&stopped)
                            && !pid.is_empty()
                        {
                            break pid;
                        }
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                })
                .await;
                cancel.cancel();
                pid.unwrap()
            }
        );
        let Reply::Mutation {
            effect,
            issue: Some(error),
            oid,
            ..
        } = mutated.unwrap()
        else {
            panic!("expected cancellation with known outcome")
        };
        assert_eq!(error.code, "cancelled");
        assert_eq!(
            effect,
            if publish {
                Effect::Applied
            } else {
                Effect::NotApplied
            }
        );
        assert_eq!(oid.is_some(), publish);
        assert_eq!(fixture.index(), index);
        assert_eq!(
            fs::read(fixture.collection.join("one")).unwrap(),
            b"to commit\n"
        );
        assert!(
            !Command::new("/bin/sh")
                .args(["-c", "kill -0 \"$1\" 2>/dev/null", "test", &pid])
                .status()
                .unwrap()
                .success()
        );
        fs::remove_file(marker).unwrap();
    }
}

#[cfg(unix)]
#[tokio::test]
async fn process_timeout_output_limit_and_sanitized_failure() {
    let fixture = Fixture::new();
    let hooks = tempfile::tempdir().unwrap();
    let executable = wrapper(&fixture, "exec /bin/sleep 25");
    let git = process::Git::new(
        &executable,
        &fixture.root,
        hooks.path(),
        CancellationToken::new(),
        Duration::from_millis(50),
    );
    assert_eq!(
        git.run(&["status"], None, None).await.err().unwrap().code,
        "timeout"
    );

    let executable = wrapper(&fixture, "exec /usr/bin/head -c 2097153 /dev/zero");
    let git = process::Git::new(
        &executable,
        &fixture.root,
        hooks.path(),
        CancellationToken::new(),
        Duration::from_secs(2),
    );
    assert_eq!(
        git.run(&["status"], None, None).await.err().unwrap().code,
        "limit"
    );

    let executable = wrapper(&fixture, "printf 'private-fixture-content' >&2; exit 7");
    let git = process::Git::new(
        &executable,
        &fixture.root,
        hooks.path(),
        CancellationToken::new(),
        Duration::from_secs(2),
    );
    let error = git.read(&["status"]).await.unwrap_err();
    assert_eq!(error.code, "process-failed");
    assert!(
        !serde_json::to_string(&error)
            .unwrap()
            .contains("private-fixture-content")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn stage_paths_rejects_any_disk_change_before_publishing_the_batch() {
    let fixture = Fixture::new();
    fixture.write("base.yml", b"original\n");
    fixture.git(&["add", "--", "collection/base.yml"]);
    fixture.git(&["commit", "-qm", "initial"]);
    fixture.write("one.yml", b"selected one\n");
    fixture.write("two.yml", b"selected two\n");
    let status = fixture.status().await;
    let original = fixture.index();
    let executable = wrapper(
        &fixture,
        r#"for arg in "$@"; do
            if [ "$arg" = add ]; then
                "$real" "$@" || exit $?
                printf 'edited outside the panel\n' > collection/two.yml
                exit 0
            fi
        done"#,
    );
    let reply = run(
        &fixture.collection,
        Action::StagePaths {
            paths: vec!["one.yml".into(), "two.yml".into()],
            revision: status.revision,
        },
        CancellationToken::new(),
        &executable,
    )
    .await
    .unwrap();
    assert!(matches!(
        reply,
        Reply::Mutation {
            effect: Effect::NotApplied,
            issue: Some(Error {
                code: "stale-state",
                ..
            }),
            ..
        }
    ));
    assert_eq!(fixture.index(), original);
    assert!(!fixture.root.join(".git/index.lock").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn concurrent_staging_between_revalidation_and_publication_preserves_external_index() {
    let fixture = Fixture::new();
    fixture.write("one", b"reviewed bytes\n");
    let status = fixture.status().await;
    let Reply::Diff {
        file_revision: Some(file_revision),
        ..
    } = fixture.diff(&status, "one", Side::Worktree).await.unwrap()
    else {
        panic!("expected file revision");
    };
    let executable = wrapper(
        &fixture,
        r#"for arg in "$@"; do
            if [ "$arg" = add ]; then
                "$real" "$@" || exit $?
                : > .git/after-private-add
                exit 0
            fi
            if [ "$arg" = hash-object ] && [ -f .git/after-private-add ]; then
                "$real" "$@" || exit $?
                oid=$(printf 'external bytes\n' | "$real" hash-object -w --stdin) || exit $?
                "$real" update-index --add --cacheinfo "100644,$oid,collection/one" || exit $?
                exit 0
            fi
        done"#,
    );
    let action = Action::Stage {
        path: "one".into(),
        revision: status.revision,
        file_revision,
        replace_partial: false,
    };
    let reply = run(
        &fixture.collection,
        action,
        CancellationToken::new(),
        &executable,
    )
    .await
    .unwrap();
    assert!(matches!(
        reply,
        Reply::Mutation {
            effect: Effect::NotApplied,
            issue: Some(Error {
                code: "stale-state",
                ..
            }),
            ..
        }
    ));
    assert_eq!(
        fixture.git(&["show", ":collection/one"]),
        b"external bytes\n"
    );
    assert_eq!(
        fs::read(fixture.collection.join("one")).unwrap(),
        b"reviewed bytes\n"
    );
    assert!(!fixture.root.join(".git/index.lock").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn global_filter_configuration_allows_ordinary_paths_without_running_the_filter() {
    let fixture = Fixture::new();
    fixture.write("ordinary", b"ordinary\n");
    fixture.write("asset", b"content\n");
    fixture.write(".gitattributes", b"asset filter=fixture\n");
    fixture.git(&[
        "config",
        "--file",
        ".git/fixture-global",
        "filter.fixture.clean",
        "printf filter > SHOULD_NOT_EXIST",
    ]);
    fixture.git(&[
        "config",
        "--file",
        ".git/fixture-global",
        "filter.fixture.required",
        "true",
    ]);
    let executable = wrapper(&fixture, "export GIT_CONFIG_GLOBAL=.git/fixture-global");
    let Reply::Status { status } = run(
        &fixture.collection,
        Action::Status,
        CancellationToken::new(),
        &executable,
    )
    .await
    .unwrap() else {
        panic!("expected status");
    };
    assert!(entry(&status, "ordinary").blocked_reason.is_none());
    assert!(
        entry(&status, "asset")
            .blocked_reason
            .unwrap()
            .contains("Git filter")
    );
    assert!(!fixture.root.join("SHOULD_NOT_EXIST").exists());
}

#[tokio::test]
async fn changing_head_after_review_requires_refresh_even_with_the_same_index() {
    let fixture = Fixture::new();
    fixture.write("one", b"initial\n");
    let status = mutation(fixture.prepare("one", false).await.unwrap());
    mutation(fixture.commit(&status, &["one"]).await.unwrap());

    fixture.write("one", b"reviewed change\n");
    let reviewed = mutation(fixture.prepare("one", false).await.unwrap());
    let index = fixture.index();
    fixture.git(&["branch", "other"]);
    fixture.git(&["symbolic-ref", "HEAD", "refs/heads/other"]);

    assert_eq!(
        fixture.commit(&reviewed, &["one"]).await.unwrap_err().code,
        "stale-state"
    );
    assert_eq!(
        fixture.git(&["rev-parse", "refs/heads/main"]),
        fixture.git(&["rev-parse", "refs/heads/other"])
    );
    assert_eq!(fixture.index(), index);
    assert_eq!(
        fs::read(fixture.collection.join("one")).unwrap(),
        b"reviewed change\n"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn only_a_confirmed_index_update_keeps_the_commit_selection() {
    let fixture = Fixture::new();
    fixture.write("already", b"first\n");
    fixture.write("one", b"selected\n");
    fixture.write("other", b"external\n");
    let first = fixture.prepare("already", false).await.unwrap();
    assert!(matches!(
        first,
        Reply::Mutation {
            selection_stable: true,
            ..
        }
    ));
    let status = mutation(first);
    let Reply::Diff {
        file_revision: Some(file_revision),
        ..
    } = fixture.diff(&status, "one", Side::Worktree).await.unwrap()
    else {
        panic!("expected a working tree revision");
    };
    let executable = wrapper(
        &fixture,
        r#"for arg in "$@"; do
            if [ "$arg" = add ]; then
                "$real" "$@" || exit $?
                : > .git/selection-stage-started
                exit 0
            fi
            if [ "$arg" = config ] && [ -f .git/selection-stage-started ] &&
                "$real" ls-files --error-unmatch -- collection/one >/dev/null 2>&1; then
                "$real" add -- collection/other || exit $?
                rm .git/selection-stage-started
            fi
        done"#,
    );
    let result = run(
        &fixture.collection,
        Action::Stage {
            path: "one".into(),
            revision: status.revision,
            file_revision,
            replace_partial: false,
        },
        CancellationToken::new(),
        &executable,
    )
    .await
    .unwrap();
    assert!(matches!(
        result,
        Reply::Mutation {
            effect: Effect::Applied,
            selection_stable: false,
            ..
        }
    ));
    let status = mutation(result);
    assert_eq!(entry(&status, "one").index, 'A');
    assert_eq!(entry(&status, "other").index, 'A');
}
