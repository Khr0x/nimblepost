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
  <img src="https://img.shields.io/badge/status-alpha%200.1.0-10b981?style=flat-square&amp;labelColor=27272a" alt="Status: alpha 0.1.0">
  <img src="https://img.shields.io/badge/Tauri-2-27272a?style=flat-square" alt="Tauri 2">
  <img src="https://img.shields.io/badge/Svelte-5-27272a?style=flat-square" alt="Svelte 5">
  <img src="https://img.shields.io/badge/core-Rust-27272a?style=flat-square" alt="Rust core">
</p>

<p>
  <a href="#what-you-can-do">Features</a> ·
  <a href="#run-the-desktop-app">Getting started</a> ·
  <a href="#try-the-included-example">Try it</a> ·
  <a href="https://github.com/Khr0x/nimblepost/issues">Report an issue</a>
</p>

</div>

NimblePost keeps your collections in your own folders, with a desktop app built
on Tauri and Svelte and a Rust engine shared by the desktop app and CLI.

**Status: ALPHA · 0.1.0.** The HTTP/HTTPS workflow is functional and under active
development. Native release validation on Windows and Linux, and performance
benchmarks, are still pending.

## What you can do

- Organize multiple collections in local workspaces, with folders and request tabs.
- Create requests from the sidebar or start an untitled request and choose where
  to save it later.
- Edit query parameters, headers, variables, environments and JSON/text/XML bodies.
- Use Basic, Bearer or API Key authentication, including inherited configuration.
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
request/response bodies. Unsaved drafts are not restored after restarting the app.

## Run the desktop app

You need Node.js 24 or later, npm, Rust with Cargo, and the native build
prerequisites for Tauri 2 on your operating system. On macOS, install Xcode
Command Line Tools.

From the repository root:

```sh
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
Ctrl+C cancels execution. It supports variable overrides and declared secrets
supplied through explicitly named process environment variables.

## Development checks

```sh
npm test
npm run check
npm run build
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Tests cover request editing, response handling, filesystem storage and
OpenCollection compatibility. HTTP integration tests use local servers.

GitHub Actions runs these checks on branch pushes and pull requests using Ubuntu 24.04,
Node.js 24 and Rust 1.96.1. The workflow can also be started manually from the
repository's Actions tab.

GraphQL, gRPC, WebSocket, imports, Git integration and collection runners are
not implemented yet.

## macOS ALPHA releases

Pushing an ALPHA tag such as `v0.1.0-alpha.1` runs the same CI checks first. If
they pass, the release workflow builds a universal macOS app for Apple Silicon
and Intel, packages a DMG and prepares a **draft prerelease** on GitHub. Review
the workflow results and test the downloaded app before publishing the draft.
The tag's base version must match the app version in the repository.

The build uses ad-hoc signing without an Apple Developer ID certificate or
notarization. macOS may block the first launch; after attempting to open the
app, users who trust the download can select **System Settings → Privacy &
Security → Open Anyway**. See [Apple's instructions](https://support.apple.com/en-us/102445).

For further usage details, see the [desktop guide](apps/desktop/README.md) and
[CLI guide](bins/cli/README.md).
