# OpenCollection compatibility fixtures

This corpus combines synthetic Apache-2.0 fixtures authored for NimblePost and
MIT-licensed upstream Bruno runner fixtures. [manifest.json](manifest.json)
records hashes, document types, expected schema results, and provenance.

| Fixture | Origin | Expected result |
| --- | --- | --- |
| `nimblepost/collection.yml` | Synthetic warehouse collection | Valid |
| `nimblepost/request.yml` | Synthetic delivery request with a comment, quoted values, and a multiline JSON body | Valid |
| `nimblepost/environment-invalid.json` | Synthetic training environment | Invalid due to `transient` |
| `bruno/opencollection.yml` | Bruno runner tests | Invalid due to header `enabled` |
| `bruno/get-users.yml` | Bruno runner tests | Valid |
| `bruno/environments/dev.yml` | Bruno runner tests | Valid |
| `bruno/users/folder.yml` | Bruno runner tests | Valid. The filesystem layout infers the type from the filename. |
| `bruno/users/create-user.yml` | Bruno runner tests | Invalid due to body `mode/json` |

The upstream Bruno files are preserved byte for byte. Tests assert the specific
schema issues listed in the manifest for invalid fixtures. The runnable
NimblePost reference collection lives separately in
[examples/basic-http](../../../examples/basic-http/README.md).

Tests validate these files without executing their URLs or scripts. Use
synthetic data when adding fixtures. Upstream values that resemble API keys
are examples from the Bruno project.

See the [third-party notices](../../../THIRD_PARTY_NOTICES.md) and
[original Bruno license](LICENSE.bruno.md) for attribution.
