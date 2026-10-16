# RFC-0004 — Contrato del motor HTTP Rust

Estado: implementado; formalizado por NP-016 el 2026-10-16 sobre `b8aa888`.
Autoridad del formato, precedencia y guardado: [RFC-0003](0003-contrato-opencollection.md).

## Entradas y salidas actuales

Los exports están en [lib.rs](../../crates/core/src/lib.rs). Las operaciones
de carga y preparación no envían red ni guardan el documento:

| Operación/símbolo | Entrada | Resultado |
|---|---|---|
| `Document::read/from_yaml` | Ruta/UTF-8 YAML | Original íntegro y proyección; `validate` comprueba `DocumentKind` |
| `load_request` | Raíz explícita, ruta relativa, environment opcional | `LoadedRequest` con defaults de colección/carpetas y environment |
| `LoadedRequest::standalone` | Documento HttpRequest | Request sin defaults ni environment implícitos |
| `LoadedRequest::in_collection` | Raíz, documento transitorio, environment | Defaults de colección y environment; no infiere carpeta de un request sin guardar |
| `prepare` | `LoadedRequest`, `ExecutionContext` | `PreparedRequest` validado; resolución solo en memoria |
| `execute` | PreparedRequest y `CancellationToken` | `HttpResponse { status, headers, body }` o error |
| `Document::edited` / `save_document` | Original, `DocumentKind`, `FieldEdit[]` / original y editado | Edición conservadora / documento guardado; operación independiente del envío |

Fuente: [collection.rs](../../crates/core/src/collection.rs),
[environment.rs](../../crates/core/src/environment.rs),
[http.rs](../../crates/core/src/http.rs), [storage.rs](../../crates/core/src/storage.rs).
`ExecutionContext` contiene mapas `overrides` y `secrets`. Precedencia:
override → petición → environment → carpeta más cercana → colección.
`VariableOrigin` expone procedencia; `inherited_variables` excluye valores de
secretos declarados. No lee implícitamente variables del proceso ni proveedores.

`HttpResponse.body` son bytes originales; headers conservan duplicados y bytes
de sus valores. No se garantiza casing ni orden de las líneas originales HTTP.
HTTP 4xx/5xx es una respuesta válida. El adaptador decide presentación/encoding;
la biblioteca no imprime ni registra payloads.

## Límites y semántica

- YAML UTF-8: 8 MiB por archivo, profundidad 64, documento único y keys únicas;
  sin aliases/anchors/tags/merge keys. Validadores locales fijados, sin descargas.
- Layout filesystem y request suelto. Bundled/mixto no es ejecutable actualmente.
- HTTP/HTTPS; body JSON/text/XML; Basic/Bearer/API Key y herencia. Configuración
  activa no soportada, variables ausentes/duplicadas o colisión auth/manual
  fallan en preparación. Sustitución de una pasada, sin eval ni recursión.
- Timeout de `execute`: conexión, redirects, headers y body; 30.000 ms por defecto.
  No incluye la carga previa del YAML. `Session` mide además trabajo del adaptador.
- `execute` observa token cancelado antes y durante red/lectura; descarta su
  future. El token no convierte todas las funciones de filesystem en cancelables.
  No revierte efectos ya ejecutados en el servidor.
- Respuesta en memoria: máximo 16 MiB, error al exceder, sin truncar. Reserva
  validada por Content-Length y crecimiento acotado también para chunked.
  No hay streaming a disco; los chunks IPC se leen después de completar HTTP.
- Redirects dentro del mismo origen, máximo 5 por defecto (configurable 0–20).
  Cross-origin devuelve 3xx sin reenviar credenciales. Proxies del sistema y
  explícitos desactivados; sin cookies persistentes ni certificados personalizados.
- Guardado: hasta 1.024 ediciones permitidas, revisión por bytes, no-op conserva
  mtime, temporal en el mismo directorio y permisos conservados. Sin transacción
  multiarchivo ni garantía contra la ventana comprobación final → reemplazo.

## Errores y privacidad

[Error](../../crates/core/src/error.rs) distingue `Io`, `Yaml`, `Config`,
`MissingVariable`, `Cancelled`, `Timeout`, `Network`, `BodyTooLarge` y `Conflict`.
`NetworkError` diferencia Connect/Redirect/Response/Transport. Los mensajes
incluyen ubicación/nombre de campo, no valores HTTP ni errores crudos de terceros.
`Conflict` puede indicar el temporal conservado. Rutas y nombres no son secretos
redactados de forma general; elegirlos puede revelar información local.

Guardar usa plantillas originales; nunca `ExecutionContext` ni el request resuelto.
El [history](../../crates/core/src/history.rs) admite 200 metadatos, sin URL,
headers/body ni valores de environment; no sobrescribe corrupción automáticamente.
Respuestas y configuración literal siguen siendo sensibles aunque no sean history.

## Verificación y ampliaciones

[engine.rs](../../crates/core/tests/engine.rs) cubre fixtures, precedencia,
secretos, variables/funciones incompatibles, binario/duplicados, HTTP real,
timeout/cancelación, límites y redirects cross-origin.
[storage.rs](../../crates/core/tests/storage.rs) cubre bytes/mtime/permisos,
unknowns, conflictos, rutas y privacidad de history. Adaptadores:
[IPC](0008-contrato-ipc-desktop.md) y [CLI](0009-contrato-cli.md).
Protocolos, streaming, runner y providers nuevos necesitan su tarea y contrato;
no constituyen trabajo pendiente para volver a implementar este motor.
