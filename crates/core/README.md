# NimblePost core

The shared Rust library reads OpenCollection YAML 1.2, validates it against
the pinned schema, resolves variables, and executes HTTP/HTTPS with Tokio,
reqwest, and rustls. It can run independently of Desktop and CLI.

```rust,no_run
use nimblepost_core::{
    CancellationToken, ExecutionContext, execute, load_request, prepare,
};

# async fn example() -> nimblepost_core::Result<()> {
let loaded = load_request("examples/basic-http", "users/list.yml", Some("local")).await?;
let context = ExecutionContext::default();
let prepared = prepare(&loaded, &context)?;
let token = CancellationToken::new(); // Another task can call token.clone().cancel().
let response = execute(prepared, &token).await?;

assert_eq!(response.status, 200);
// response.headers preserves duplicates. response.body contains the original bytes.
# Ok(())
# }
```

For a standalone file, use `Document::read(path).await` followed by
`LoadedRequest::standalone(document)`. Supply its variables through
`ExecutionContext.overrides`. A `Document` keeps its full `original()` source
available even when `validate()` finds an incompatible field.

Variable precedence, highest first, is transient overrides, request,
environment, folders from nearest to farthest, and collection.
`variable_origins()` returns the source scope and file without exposing values.
Pass declared secrets through `ExecutionContext.secrets`. Only referenced
secrets are required. Preparation and execution leave source files untouched.

The engine supports headers, query parameters, JSON/text/XML bodies, Basic,
Bearer and API Key auth, and inherited defaults from collections and folders.
Active settings outside the supported subset produce an error before sending.

`load_defaults(root, folder)` reads the scope and its ancestors without writing,
even if `folder.yml` is absent. `Document::edited` supports collection and folder
headers, auth, and variables. `create_folder_defaults` creates missing metadata
exclusively after validating edits. Use `save_document` for existing documents.
It refuses to recreate a file deleted externally. See the
[defaults editor](../../apps/desktop/README.md#collection-and-folder-defaults).

## Run the example

With a local HTTP server listening on port 3000, run the example from the root.

```sh
cargo run -p nimblepost-core --example send -- examples/basic-http users/list.yml local
```

To override the base URL without editing YAML, pass it as the final argument.

```sh
cargo run -p nimblepost-core --example send -- examples/basic-http users/list.yml local http://127.0.0.1:8080
```

The example prints status, headers, and a text representation of the body as
readable JSON. The library returns original bytes, including binary responses.
Ctrl+C cancels execution. The [CLI](../../bins/cli/README.md) adds named arguments
and base64 output for binary bodies.

## Timeouts, cancellation, and errors

The end-to-end timeout covers connection, redirects, headers, and body reads.
It defaults to 30,000 ms and accepts a positive `settings.timeout` override.
A cancelled `CancellationToken` prevents the request from starting. Cancelling
during execution drops the HTTP future. Server-side effects may already have
occurred.

Typed errors distinguish unreadable files, revision conflicts, invalid YAML
with line and column, invalid configuration with file and field, missing
variables, timeouts, cancellation, connection and transport errors, and body
size limits. Raw parser and network errors can contain URLs or credentials,
so the library reports typed errors with its own messages. HTTP 4xx and 5xx
are valid responses.

## Supported subset

- Filesystem collections and standalone requests. `collection_index()` finds
  the tree and environments, and `request_info()` exposes saved templates.
  Desktop and CLI share this library. Bundled collections and the runner are
  planned.
- YAML up to 8 MiB and 64 levels deep. Aliases, anchors, tags, merge keys,
  duplicate keys, and multiple documents are rejected. Schema validation runs
  locally.
- Response bodies up to 16 MiB in memory. Desktop retrieves 64 KiB chunks via
  IPC. Streaming to a file is planned.
- The engine disables explicit and system proxies. Persistent cookies, OAuth,
  scripts, assertions, and custom certificates are outside the supported
  subset.
- The engine follows redirects within the original origin up to `maxRedirects`.
  A change of origin returns the original 3xx to protect credentials in custom
  headers and query parameters.
- `encodeUrl: false` and manually set framing or transport headers are rejected.
  reqwest handles encoding and sets transport headers such as Host and
  Content-Length.
- `Document::edited(kind, edits)` edits field paths using the concrete syntax
  tree from yaml-edit 0.3.2. New nodes may use JSON flow syntax (valid YAML 1.2).
  It checks that only requested values changed and that edits introduce no
  new schema errors.
- `save_document(original, edited)` compares bytes, preserves permissions,
  syncs a temporary file, and replaces the original. A no-op save preserves
  modification time. Backend writers are serialized. External writers can
  still race between the final check and rename, and a conflict preserves the
  temporary file if it was already written.
- `History` stores bounded execution metadata and excludes HTTP payloads, URLs,
  and credentials. Desktop uses its own
  [history and secrets policy](../../apps/desktop/README.md#mvp-0-secrets-and-history-policy).

```sh
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Integration tests use ephemeral servers on `127.0.0.1`. The project is
developed and tested with Rust 1.96.1.
