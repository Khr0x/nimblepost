# Third-party notices

NimblePost's own code is licensed under [Apache-2.0](LICENSE). The material
listed below retains its upstream licensing and attribution.

## OpenCollection schema

- Local file: `schemas/opencollection/1.0.0/opencollection.schema.json`.
- Source: [OpenCollection schema, revision 535a5c5](https://github.com/opencollection-dev/opencollection/blob/535a5c5803da7a3fe1d575fff23350b4a7c43c6a/packages/oc-schema/src/opencollection.schema.json).
- Package: `@opencollection/schema` 0.14.0, authored by Bruno Software Inc.
- License: MIT, as declared in the [upstream package manifest](https://github.com/opencollection-dev/opencollection/blob/535a5c5803da7a3fe1d575fff23350b4a7c43c6a/packages/oc-schema/package.json).

The vendored schema is unchanged. Its pinned hash is recorded in the
[fixture manifest](tests/fixtures/opencollection/manifest.json). The schema
snapshot contains no copyright header; the author attribution above comes
from the package manifest. The MIT permission terms are reproduced below.

> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

## OpenCollection documentation examples

`tests/fixtures/opencollection/official/` contains examples extracted from
OpenCollection's documentation at revision
`535a5c5803da7a3fe1d575fff23350b4a7c43c6a`. The
[fixture manifest](tests/fixtures/opencollection/manifest.json) records each
source URL, extraction method and hash. The
[documentation package manifest](https://github.com/opencollection-dev/opencollection/blob/535a5c5803da7a3fe1d575fff23350b4a7c43c6a/packages/oc-spec-site/package.json)
does not declare a separate license. This attribution does not assign a new
license to those upstream examples.

## Bruno compatibility fixtures

`tests/fixtures/opencollection/bruno/` contains unchanged fixtures from
[Bruno](https://github.com/usebruno/bruno/tree/bc90e46b288446ebe0c8d67b28470d2efc0a8c48),
revision `bc90e46b288446ebe0c8d67b28470d2efc0a8c48`.

Copyright (c) 2022 Anoop M D, Anusree P S and Contributors.
The original [MIT license](tests/fixtures/opencollection/LICENSE.bruno.md)
is included alongside the fixtures. Source paths and hashes are in the
[fixture manifest](tests/fixtures/opencollection/manifest.json).

## shadcn-svelte components

`apps/desktop/src/lib/components/ui/` contains components derived from
shadcn-svelte. Copyright (c) 2023 Hunter Johnston, CokaKoala and shadcn.
The original [MIT license and copyright notices](apps/desktop/src/lib/components/ui/LICENSE.md)
are retained with the components.

## Inter font

`apps/desktop/src/assets/fonts/inter-latin-wght-normal.woff2` is Inter.
Copyright (c) 2016 The Inter Project Authors.
The [SIL Open Font License 1.1](apps/desktop/src/assets/fonts/OFL.txt)
is retained alongside the font and included in desktop bundle resources.

## Package dependencies

JavaScript and Rust dependency versions are recorded in `package-lock.json`
and `Cargo.lock`. Their upstream licenses remain applicable. This file records
vendored material and is not a complete dependency SBOM.
