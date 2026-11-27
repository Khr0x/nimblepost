use super::{Error, Result};
use nimblepost_core::CancellationToken;
use std::{
    collections::BTreeSet,
    env,
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    time::Instant,
};

pub(super) const MAX_BYTES: usize = 2 * 1024 * 1024;

pub(super) fn executable() -> Result<PathBuf> {
    let name = if cfg!(windows) { "git.exe" } else { "git" };
    env::split_paths(&env::var_os("PATH").unwrap_or_default())
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|file| file.is_file())
        .ok_or_else(Error::unavailable)
}

pub(super) struct Git<'a> {
    pub executable: &'a Path,
    pub directory: &'a Path,
    pub hooks: &'a Path,
    pub token: CancellationToken,
    stop: Instant,
    bytes: AtomicUsize,
    filters: Mutex<BTreeSet<String>>,
}

pub(super) struct Output {
    pub code: Option<i32>,
    pub bytes: Vec<u8>,
    stderr: Vec<u8>,
}

impl Output {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    pub fn checked(self) -> Result<Vec<u8>> {
        if self.success() {
            return Ok(self.bytes);
        }
        let stderr = String::from_utf8_lossy(&self.stderr);
        if stderr.contains("cannot lock ref") && stderr.contains("but expected") {
            Err(Error::stale())
        } else if stderr.contains("unknown option") || stderr.contains("is not a git command") {
            Err(Error::new(
                "unsupported",
                "The system Git lacks a required command or option. Update Git.",
            ))
        } else if stderr.contains(".lock") && stderr.contains("File exists") {
            Err(Error::new("busy", "Git is holding a repository lock."))
        } else {
            Err(Error::new(
                "process-failed",
                "Git could not complete the operation.",
            ))
        }
    }
}

impl<'a> Git<'a> {
    pub fn new(
        executable: &'a Path,
        directory: &'a Path,
        hooks: &'a Path,
        token: CancellationToken,
        duration: Duration,
    ) -> Self {
        Self {
            executable,
            directory,
            hooks,
            token,
            stop: Instant::now() + duration,
            bytes: AtomicUsize::new(0),
            filters: Mutex::new(BTreeSet::new()),
        }
    }

    pub fn reconcile(&self) -> Self {
        Self::new(
            self.executable,
            self.directory,
            self.hooks,
            CancellationToken::new(),
            Duration::from_secs(2),
        )
    }

    pub fn set_filters(&self, filters: BTreeSet<String>) {
        *self.filters.lock().unwrap() = filters;
    }

    pub async fn bound<T>(&self, future: impl Future<Output = Result<T>>) -> Result<T> {
        tokio::select! {
            biased;
            _ = self.token.cancelled() => Err(Error::new("cancelled", "Git operation cancelled.")),
            _ = tokio::time::sleep_until(self.stop) => Err(Error::new("timeout", "Git operation timed out.")),
            result = future => result,
        }
    }

    pub async fn read(&self, args: &[&str]) -> Result<Vec<u8>> {
        self.run(args, None, None).await?.checked()
    }

    pub async fn run(
        &self,
        args: &[&str],
        input: Option<&[u8]>,
        index: Option<&Path>,
    ) -> Result<Output> {
        self.bound(std::future::ready(Ok(()))).await?;
        let mut command = Command::new(self.executable);
        command
            .current_dir(self.directory)
            .env_clear()
            .env("LC_ALL", "C")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("GIT_ALLOW_PROTOCOL", "")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .args([
                "--no-pager",
                "--literal-pathspecs",
                "--git-dir=.git",
                "--work-tree=.",
            ])
            .args([
                "-c",
                "core.fsmonitor=false",
                "-c",
                "core.untrackedCache=false",
                "-c",
                "core.quotePath=false",
                "-c",
                "user.useConfigOnly=true",
                "-c",
                "diff.autoRefreshIndex=false",
                "-c",
                "index.skipHash=false",
            ]);
        // Only the config query needs to see the configured hook path and signing policy.
        if args.first() != Some(&"config") {
            let mut hooks = OsStr::new("core.hooksPath=").to_os_string();
            hooks.push(self.hooks);
            command
                .args(["-c", "commit.gpgSign=false"])
                .arg("-c")
                .arg(hooks);
            for driver in &*self.filters.lock().unwrap() {
                for setting in ["clean=", "process=", "smudge=", "required=false"] {
                    command.arg("-c").arg(format!("{driver}.{setting}"));
                }
            }
        }
        for key in [
            "PATH",
            "HOME",
            "USERPROFILE",
            "HOMEDRIVE",
            "HOMEPATH",
            "XDG_CONFIG_HOME",
            "SYSTEMROOT",
            "WINDIR",
            "TEMP",
            "TMP",
            "TMPDIR",
            "PROGRAMDATA",
        ] {
            if let Some(value) = env::var_os(key) {
                command.env(key, value);
            }
        }
        if let Some(index) = index {
            command.env("GIT_INDEX_FILE", index);
        }
        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        let mut child = command
            .args(args)
            .kill_on_drop(true)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| match error.kind() {
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => Error::unavailable(),
                _ => Error::io(),
            })?;
        let stdout = child.stdout.take().expect("Git stdout is piped");
        let stderr = child.stderr.take().expect("Git stderr is piped");
        let stdin = child.stdin.take();
        let result = self
            .bound(async {
                let send = async {
                    if let (Some(mut stdin), Some(input)) = (stdin, input)
                        && let Err(error) = stdin.write_all(input).await
                        && error.kind() != io::ErrorKind::BrokenPipe
                    {
                        return Err(Error::io());
                    }
                    Ok(())
                };
                let (bytes, stderr, status, ()) = tokio::try_join!(
                    self.drain(stdout),
                    self.drain(stderr),
                    async { child.wait().await.map_err(|_| Error::io()) },
                    send,
                )?;
                Ok(Output {
                    code: status.code(),
                    bytes,
                    stderr,
                })
            })
            .await;
        if result.is_err() {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
        result
    }

    async fn drain(&self, mut pipe: impl AsyncRead + Unpin) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        let mut chunk = vec![0; 8192];
        loop {
            let size = pipe.read(&mut chunk).await.map_err(|_| Error::io())?;
            if size == 0 {
                return Ok(output);
            }
            if self.bytes.fetch_add(size, Ordering::Relaxed) + size > MAX_BYTES {
                return Err(Error::limit());
            }
            output.extend_from_slice(&chunk[..size]);
        }
    }
}
