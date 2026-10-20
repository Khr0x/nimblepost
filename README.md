<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="apps/desktop/src/assets/nimblepost-glyph-dark.svg">
  <source media="(prefers-color-scheme: light)" srcset="apps/desktop/src/assets/nimblepost-glyph-light.svg">
  <img src="apps/desktop/src/assets/nimblepost-glyph-light.svg" alt="NimblePost logo" width="64" height="64">
</picture>

<h1>NimblePost</h1>

<p><strong>Built to move fast. Designed to stay light.</strong></p>
<p>An open-source, local-first Postman alternative for desktop and CLI.</p>

<p>
  <a href="https://github.com/Khr0x/nimblepost/actions/workflows/ci.yml"><img src="https://github.com/Khr0x/nimblepost/actions/workflows/ci.yml/badge.svg" alt="CI status"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue?style=flat-square" alt="License: Apache 2.0"></a>
  <img src="https://img.shields.io/badge/status-alpha-10b981?style=flat-square&amp;labelColor=27272a" alt="Status: alpha">
  <img src="https://img.shields.io/badge/Tauri-2-27272a?style=flat-square" alt="Tauri 2">
  <img src="https://img.shields.io/badge/Svelte-5-27272a?style=flat-square" alt="Svelte 5">
  <img src="https://img.shields.io/badge/core-Rust-27272a?style=flat-square" alt="Rust core">
</p>

<p>
  <a href="#what-you-can-do">Features</a> ·
  <a href="#run-the-desktop-app">Getting started</a> ·
  <a href="#try-the-included-example">Try it</a> ·
  <a href="#contributing">Contributing</a> ·
  <a href="#license">License</a> ·
  <a href="https://github.com/Khr0x/nimblepost/issues">Report an issue</a>
</p>

</div>

NimblePost is an open-source API client for sending HTTP requests, inspecting
responses and keeping collections in files you control. The desktop app uses
Tauri 2 and Svelte 5; a shared Rust engine powers both the app and the CLI.
Collections are plain OpenCollection YAML, so you can review them in an editor
and version them with your own Git tools.

**Status: ALPHA.** The HTTP/HTTPS workflow is functional and under active
development. Public installers target macOS. Windows and Linux x64 have
automated package installation, startup and removal checks; their broader
interactive acceptance is still incomplete. Performance targets are not
guarantees. See [current limitations](#current-limitations) before adopting it.

## What you can do

- Organize multiple collections in local workspaces, with folders and request tabs.
- Refresh the tree automatically after external editor or Git changes, while
  keeping edited requests protected with reload and save-copy options.
- Create requests from the sidebar or start an untitled request and choose where
  to save it later.
- Edit query parameters, headers, variables, environments and JSON/text/XML bodies.
- Use Basic, Bearer or API Key authentication, including inherited configuration.
- Edit collection and folder defaults with source information, explicit Save/Discard
  and protection against external changes.
- Send requests, cancel execution and inspect response status, timing and headers.
- Read JSON with formatting, syntax highlighting and collapsible objects and
  arrays, or switch to Text, Raw or Hex views.
- Work in dark mode by default, switch to light mode and resize the sidebar and
  response panel.
- Execute a saved request from the CLI using the same Rust engine.

## Your data stays local

Collections use OpenCollection 1.0.0 YAML files. You choose their location;
NimblePost does not require an account or cloud service. Workspace references
and execution history are stored in the app's local data directory.

Saving preserves comments and fields outside the edited sections, and detects
external file changes before overwriting. Removing a collection from a workspace
or deleting a workspace keeps its collection files on disk.

Declared secrets can be supplied in memory for execution. Use variable templates
such as `{{token}}` in saved requests. History retains metadata for the last
200 executions and excludes URLs, credentials, variable values, headers and
request/response bodies. Request tabs and unsaved drafts are restored after
restarting from a separate local recovery file. Runtime secrets and responses
are excluded from that backup.

## Install

Download the macOS ALPHA DMG from [GitHub Releases](https://github.com/Khr0x/nimblepost/releases).
The universal app includes Apple Silicon and Intel builds; execution on real
Intel hardware has not yet been verified.

The app uses ad-hoc signing and is not notarized with an Apple Developer ID.
If macOS blocks a download you trust, follow
[Apple's instructions for opening an app](https://support.apple.com/en-us/102445).
For other platforms, build from source using the instructions below.

## Run the desktop app

You need Node.js 24 or later, npm, Rust with Cargo, and the
[native prerequisites for Tauri 2](https://v2.tauri.app/start/prerequisites/)
on your operating system. On macOS, install Xcode Command Line Tools. CI uses
Rust 1.96.1; use that toolchain to match its checks.

Clone the repository and install its JavaScript dependencies:

```sh
git clone https://github.com/Khr0x/nimblepost.git
cd nimblepost
npm ci
npm run desktop:dev
```

To create your first request:

1. Choose **New collection**, enter a name and select its location.
2. Choose **New request** in the collection. Only the name is required at creation.
3. Enter the URL, select the method and configure the request.
4. Click **Send** to inspect the response and **Save** to persist your edits.

You can also use **Open collection** to select an existing folder containing
`opencollection.yml`. The **+** button in the tab bar starts an untitled request;
its first save asks for a name, collection and destination folder.

To compile the native executable without creating an installer:

```sh
npm run desktop:build
```

## Try the included example

Start the demo API in a separate terminal:

```sh
node examples/basic-http/server.mjs
```

It listens on `http://127.0.0.1:3000`. In the desktop app, choose **Try example**
to open the included collection in read-only mode.

To execute the same saved request from the CLI:

```sh
cargo run -p nimblepost-cli -- run users/list.yml --collection examples/basic-http --env local
```

The CLI executes one request per invocation and prints the response as JSON.
Ctrl+C cancels execution. It supports repeatable `--var NAME=VALUE` overrides
and declared secrets supplied through `--secret-env NAME=ENV_VAR` without
putting the secret value in command-line arguments. HTTP 4xx/5xx responses are
successful executions (exit code 0); load or transport failures return 1,
invalid arguments return 2, and cancellation returns 130.

To build the CLI binary separately:

```sh
cargo build -p nimblepost-cli --release --locked
```

The executable is `target/release/nimblepost` (`nimblepost.exe` on Windows).
See the [CLI guide](bins/cli/README.md) for standalone requests and secrets.

## Development checks

```sh
npm test
npm run check
npm run build
npm run release:check
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Tests cover request editing, response handling, filesystem storage and
OpenCollection compatibility. HTTP integration tests use local servers.

GitHub Actions runs these checks on branch pushes and pull requests using Ubuntu 24.04,
Node.js 24 and Rust 1.96.1. The workflow can also be started manually from the
repository's Actions tab.

Native package validation and performance workflows run manually from
GitHub Actions. Passing the regular CI checks alone does not establish native
package acceptance or performance on every platform.

## Current limitations

- Only the filesystem layout of OpenCollection 1.0.0 and the supported HTTP
  subset are available; arbitrary collections may contain unsupported fields.
- Request bodies support JSON, text and XML. Multipart, binary uploads,
  persistent cookies, OAuth, custom proxies and client certificates are unavailable.
- Responses are held in memory with a 16 MiB limit. Larger responses fail instead
  of being silently truncated; streaming to disk is unavailable.
- Secrets can be supplied for execution in memory; there is no persistent
  operating-system keychain integration. Literal values saved in requests or
  drafts remain local file content, so keep credentials in secret variables.
- There are no importers, integrated Git operations, scripts/assertions,
  collection runners, GraphQL, gRPC or WebSocket support.

## Repository layout

| Path | Purpose |
| --- | --- |
| `apps/desktop` | Svelte UI, Tauri backend and desktop unit tests |
| `crates/core` | Shared Rust HTTP engine and filesystem storage |
| `bins/cli` | Command-line adapter for executing a saved request |
| `examples/basic-http` | Demo API and example collection |
| `schemas` / `tests` | Pinned OpenCollection schema, fixtures and contract tests |
| `scripts` / `.github/workflows` | Release tooling, validation and CI |

## Contributing

Bug reports, documentation fixes and focused pull requests are welcome.
For bugs, include your OS, app version, reproduction steps and expected versus
actual behavior. Remove credentials and private API data from examples and logs.
Use [GitHub Issues](https://github.com/Khr0x/nimblepost/issues) to discuss larger
changes before implementing them.

For a pull request, fork the repository, create a branch and keep the change
focused. Add tests when behavior changes, run the development checks above and
explain what changed and how you verified it. Keep the shared execution logic
in `crates/core` so desktop and CLI behavior stays consistent.

Include a Changeset for user-facing behavior changes; documentation-only
changes do not need one. See the [release guide](.changeset/README.md).
Contributions are accepted under the project's Apache-2.0 license.

## macOS ALPHA releases

Releases use Changesets. Run `npm run changeset` with a user-facing change and
commit the generated release note alongside it. After the changes reach `main`,
automation opens or updates a version PR with the changelog and synchronized
desktop, Tauri and Rust versions.

Merging that version PR runs CI, builds a universal macOS app for Apple Silicon
and Intel, and verifies its signature and DMG. It then creates the version tag
and publishes the GitHub prerelease with the installer and release notes.
Tags are generated automatically; ordinary source pushes only update the
version PR. The [release guide](.changeset/README.md) covers the one-time GitHub
setup and local checks.

The build uses ad-hoc signing without an Apple Developer ID certificate or
notarization. macOS may block the first launch; after attempting to open the
app, users who trust the download can select **System Settings → Privacy &
Security → Open Anyway**. See [Apple's instructions](https://support.apple.com/en-us/102445).

For further usage details, see the [desktop guide](apps/desktop/README.md) and
[CLI guide](bins/cli/README.md).

## License

NimblePost is licensed under the [Apache License 2.0](LICENSE).

See [third-party notices](THIRD_PARTY_NOTICES.md) for vendored material and
source attribution. Third-party material retains its own licensing, including
[Inter fonts](apps/desktop/src/assets/fonts/OFL.txt),
[shadcn-svelte components](apps/desktop/src/lib/components/ui/LICENSE.md) and
[Bruno test fixtures](tests/fixtures/opencollection/LICENSE.bruno.md).
