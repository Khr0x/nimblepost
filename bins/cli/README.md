# NimblePost CLI

The `nimblepost` binary reads and runs one request with `nimblepost-core`.
From the repository root, start the demo API in a separate terminal.

```sh
node examples/basic-http/server.mjs
```

Run the same YAML that Desktop opens.

```sh
cargo run -p nimblepost-cli -- run users/list.yml --collection examples/basic-http --env local
```

Use `--var NAME=VALUE` for transient overrides. Repeat it with a distinct name
for each value. For a standalone request, omit `--collection` and supply its
variables directly.

```sh
cargo run -p nimblepost-cli -- run examples/basic-http/users/list.yml --var baseUrl=http://127.0.0.1:3000 --var pageSize=5
```

For standalone requests, the CLI loads no collection defaults, environments, or
folder variables. JSON output contains `status`, `headers`, `body`,
`bodyEncoding`, and `bodyBytes`. Duplicate headers are preserved, and each
header's original value is available as `valueBase64`. The body is text for
UTF-8 responses and base64 otherwise.

Ctrl+C cancels the request. Exit codes are 0 for an HTTP response (including
4xx and 5xx), 1 for a load, preparation, or network failure, 2 for invalid
arguments, and 130 for cancellation. See the [core guide](../../crates/core/README.md)
for timeouts and limits.

For an environment declaration such as `{name: token, secret: true, type: string}`,
`--secret-env token=NIMBLEPOST_TOKEN` supplies the value of the named process
environment variable in memory. The variable must exist. The CLI reads only
the environment variables you name and keeps secret values out of both the
command-line arguments and YAML.

```sh
cargo run -p nimblepost-cli -- run users/create.yml --collection examples/basic-http --env local --secret-env token=NIMBLEPOST_TOKEN
```

Desktop provides the editor, saving, and history, and shares the same engine
and storage library. Collection-wide execution is planned.
