# Reference HTTP collection

This NimblePost example conforms to the pinned OpenCollection schema and
includes a local demo server with no extra dependencies.

`opencollection.yml` sets collection defaults. `environments/local.yml` sets
`baseUrl` and declares a secret without storing its value.
`users/list.yml` overrides the folder's `pageSize` and sends
`GET /users?limit=5`. Supply `token` in memory before running `users/create.yml`.

The files exercise comments, quoted strings, defaults, `inherit`, JSON bodies,
and an unrelated extension that should survive edits. From the repository
root, `npm test` validates the collection.

Start the server from the repository root.

```sh
node examples/basic-http/server.mjs
```

In another terminal, send the request.

```sh
cargo run -p nimblepost-cli -- run users/list.yml --collection examples/basic-http --env local
```

To use Desktop, run `npm run desktop:dev` and select **Try example**. The
server listens on `127.0.0.1:3000` and returns users as JSON. Ctrl+C stops it.
