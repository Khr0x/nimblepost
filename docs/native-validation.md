# Windows/Linux x64: native distribution acceptance

NP-010 requires both the automated package lifecycle and the interactive checks
below. Backend tests do not prove that the installed UI can perform those actions.

## Automated checks

This workflow is started manually: **Actions → Native distribution validation →
Run workflow → select the branch → Run workflow**. The workflow must exist on the
default branch for GitHub to offer the manual run button. Record the resolved SHA
from the run, not just the branch name.

Push/PR CI keeps frontend checks and Rust formatting, Clippy and tests. It does not
build or exercise Windows/Linux installers. The release workflow also reuses that
lighter CI; run native distribution validation for the package source SHA before
accepting Windows/Linux packages for release. Keep the interactive checklist below
as a separate acceptance requirement.

`.github/workflows/native-validation.yml` runs on Windows 2025 and Ubuntu 24.04
x64. It tests the Rust workspace, builds production installers without validation
hooks, checks the packaged example, installs, launches and uninstalls the package.
The executable must disappear and a synthetic external collection must retain
the same file hashes. Windows checks a native window; Linux checks a live process
under Xvfb, which does not prove that the UI rendered correctly.

The workflow uploads `native-evidence-<OS>-<arch>` for 30 days with environment,
SHA/version, backend output and lifecycle logs. Installer artifacts retain the
existing 14-day policy. These are CI artifacts, not public Windows/Linux releases.
Keep the downloaded artifact digest and the package hash with acceptance results.

Run the workflow on a branch containing the changes and record its SHA, run URL
and both job results. A successful older run cannot verify a modified workflow.
The workflow does not drive the interactive UI or certify performance targets.

## Interactive checks on each platform

Use an actual x64 Windows/Linux test machine or VM with a disposable user profile
and no personal collections. Do not reuse or overwrite a real profile. Record OS,
architecture, WebView version, package SHA-256, source SHA and app version. For
Linux, record the display/session type as well. Docker on an ARM host is not a
substitute for native x64 acceptance.

Download the installer from the recorded run. Install normally, and confirm the
app version and native window. Use a synthetic collection outside the installation
directory. Save logs/screenshots containing only synthetic data.

For local HTTP, the existing demo (`node examples/basic-http/server.mjs`) serves
`http://127.0.0.1:3000/users`. For cancellation, save the following as
`slow-server.mjs` in the temporary test directory and run `node slow-server.mjs`:

```js
import { createServer } from 'node:http';
createServer((request, response) => {
  const timer = setTimeout(() => {
    response.writeHead(200, { 'Content-Type': 'application/json' });
    response.end('{"ok":true}');
  }, 15000);
  response.on('close', () => clearTimeout(timer));
}).listen(3001, '127.0.0.1');
```

| Scenario | Action | Required observation |
|---|---|---|
| Bundled example | Open **Try example** | Tree and request load; saving the bundled example is blocked as read-only |
| Creation | Create collection and request in the temporary external directory | YAML exists; UI shows the request without errors |
| HTTP | Send GET `http://127.0.0.1:3000/users` | Status 200, JSON users, headers, timing and size visible |
| Save/reopen | Add a query row and header; edit body in a separate POST request; Save, close and reopen | Configured fields survive; no duplicate query rows or lost body |
| Cancellation | Send GET `http://127.0.0.1:3001/`, then Cancel before 15 seconds | Execution stops; controls recover; next normal GET succeeds |
| Draft recovery | Edit a saved request without saving, close/reopen the app using its recovery flow | Draft and tab restore; YAML still contains the last explicit Save |
| External conflict | Edit the request file externally while its UI draft is dirty | Reload/save-copy offered; dirty edits not silently overwritten |
| Privacy | Supply a declared synthetic runtime secret; execute once | Its unique value absent from YAML, history and recovery files; do not use literal credentials in drafts |
| Uninstall | Close app, uninstall with the platform package manager/uninstaller | App executable removed; external collection hashes unchanged |

On Windows, use the generated `uninstall.exe` or Installed Apps. For unattended
NSIS verification, `/S _?=<absolute install directory>` waits on the original
uninstaller process; `_?=` is last and its path is unquoted, as specified by the
[NSIS manual](https://nsis.sourceforge.io/Docs/Chapter3.html#3.2.2).
On Linux, obtain the actual package name using `dpkg-deb --field <file.deb> Package`
and remove that package; do not assume the name matches the executable.

Do not erase the test collection until comparing pre/post-uninstall hashes.
Uninstall acceptance does not require deleting user collections or app recovery
data. Stop only the local test servers you started.

## Evidence and closing

For each row record **pass / fail / not executed**, platform, expected/observed
result and evidence location. Include the actual uninstall exit code and hash
comparison. Mark NP-010 verified only when both platforms satisfy every criterion;
record unavailable environments as a pending prerequisite, never as a pass.

The planning record and evidence are maintained in
`/Users/cristianmendez/Documents/obsidian-khrox/nimblepost`, task NP-010.
