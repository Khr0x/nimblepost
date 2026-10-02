# Fixtures de compatibilidad

Este corpus contiene fuentes reales y ejemplos publicados, no solo YAML creado
por NimblePost. [manifest.json](manifest.json) registra origen, revisión, hash,
tipo de documento y resultado esperado contra el esquema fijado.

| Grupo | Procedencia | Resultado esperado |
|---|---|---|
| `official/collection.yml` | Literal publicado en la sección Collection | Válido |
| `official/httprequest.yml` | Literal publicado en la sección HttpRequest | Válido |
| `official/environment.json` | Objeto publicado en Environments, transcrito a JSON | Inválido: `transient` |
| `bruno/opencollection.yml` | Colección real de tests del runner | Inválido: header `enabled` |
| `bruno/get-users.yml` | Request real del runner | Válido |
| `bruno/environments/dev.yml` | Environment real del runner | Válido |
| `bruno/users/folder.yml` | Folder real del runner | Válido; el tipo se infiere del filename en filesystem |
| `bruno/users/create-user.yml` | POST real del runner | Inválido: body `mode/json` |

Los archivos de Bruno se copian byte a byte. Los ejemplos de Collection y
HttpRequest se extraen literalmente del código de la documentación. La entrada
Environment conserva campos y valores del objeto JS publicado; JSON permite
validarlo también como YAML sin añadir una conversión no documentada.

Los fixtures incompatibles son resultados **esperados**, no fallos ignorados.
Las pruebas deben detectar sus campos concretos; no basta cualquier error.
Los ejemplos propios válidos están separados en `examples/basic-http`.

No se ejecutan las URLs ni los scripts de estos fixtures. Los valores semejantes
a API keys son ejemplos públicos upstream. No se debe agregar información local.

Licencias y atribución: [THIRD_PARTY_NOTICES](../../../THIRD_PARTY_NOTICES.md)
y [licencia original de Bruno](LICENSE.bruno.md).
