# Pinned OpenCollection schema

`opencollection.schema.json` is an unchanged copy of the published
OpenCollection schema.

- OpenCollection `1.0.0` uses JSON Schema draft-07.
- The source package is `@opencollection/schema` `0.14.0`.
- The pinned revision is `535a5c5803da7a3fe1d575fff23350b4a7c43c6a`.
- [Upstream source](https://github.com/opencollection-dev/opencollection/blob/535a5c5803da7a3fe1d575fff23350b4a7c43c6a/packages/oc-schema/src/opencollection.schema.json).
- The [fixture manifest](../../../tests/fixtures/opencollection/manifest.json)
  records the SHA-256 digest, which `npm test` checks.
- The [upstream package manifest](https://github.com/opencollection-dev/opencollection/blob/535a5c5803da7a3fe1d575fff23350b4a7c43c6a/packages/oc-schema/package.json)
  declares the MIT license.
  The declared author is Bruno Software Inc. The snapshot has no separate
  license file or copyright header in the schema. See the
  [third-party notices](../../../THIRD_PARTY_NOTICES.md).

When updating the schema, review compatibility, update the revision and hash,
and rerun every fixture. Different schema revisions can share the same
`1.0.0` format version, so the package version and commit are pinned as well.

Validation uses the local schema. After `npm ci` installs the development
dependencies, `npm test` can run offline.
