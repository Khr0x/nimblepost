# RFC-0008 — Contrato interno IPC de Desktop

Estado: implementado; formalizado por NP-016 el 2026-10-16 sobre `b8aa888`.
No es una API pública estable. Fuente: [registro](../../apps/desktop/src-tauri/src/lib.rs),
[commands.rs](../../apps/desktop/src-tauri/src/commands.rs),
[Session y DTOs](../../apps/desktop/src-tauri/src/session.rs) y
[puente TypeScript](../../apps/desktop/src/lib/desktop/api.ts). `App.svelte`
consume ese puente; JavaScript no escribe la colección ni ejecuta HTTP directamente.

## Inventario de comandos registrados

Argumentos de `invoke` en camelCase; `input` y `snapshot` son objetos anidados.
Los nombres siguientes son literales del registro Tauri; no hay comandos Git/imports.

| Comando | Argumentos | Retorno correcto |
|---|---|---|
| `read_workspace` | — | WorkspaceView |
| `refresh_collections` | — | WorkspaceView, conserva selección/revisiones |
| `inspect_request` | root, path, revision | RequestInspection |
| `read_recovery` | — | RecoveryView |
| `save_recovery` | snapshot | void |
| `create_workspace` | name | WorkspaceView |
| `select_workspace` | id | WorkspaceView |
| `rename_workspace` | id, name | WorkspaceView |
| `remove_workspace` | id | WorkspaceView |
| `select_collection` | root | CollectionView |
| `remove_collection` | root | WorkspaceView; solo quita referencia |
| `choose_collection` | — | CollectionView o null si cancela el picker |
| `open_example` | — | CollectionView de solo lectura |
| `create_collection` | name, folder | CollectionView o null si cancela picker |
| `create_request` | input: CreateRequestInput | RequestUpdate |
| `create_folder` | parent, name, folder | CollectionView |
| `rename_folder` | folder, name | CollectionView |
| `rename_request` | input: RenameRequestInput | RequestUpdate |
| `delete_request` | revision | CollectionView |
| `duplicate_request` | input: RenameRequestInput | RequestUpdate |
| `read_request` | path | RequestView |
| `read_request_summaries` | root, paths | Mapa path → RequestSummary |
| `send_request` | input: SendInput | ResponseMeta |
| `cancel_request` | — | void |
| `read_response` | id, offset | Base64 de hasta 64 KiB originales |
| `release_response` | id | void |
| `save_document` | input: SaveInput | RequestView con revisión nueva |
| `read_environment` | name | RequestView |
| `read_variable_context` | path nullable, environment nullable | VariablePreview[] |
| `create_environment` | name | RequestView |
| `read_history` | — | HistoryView |
| `clear_history` | — | void |

## DTOs y estados

- `CollectionView` (`Collection` en TS): name/root, rutas requests/folders,
  environments, readOnly y warning. `WorkspaceView`: workspaces, activeWorkspaceId,
  collections, activeCollection, warning/watchWarning. `validationPlan` solo existe
  en builds instrumentados, no es parte del flujo normal del producto.
- `RequestView` (`RequestInfo` en TS): name/method/url son plantillas,
  revision, document (proyección JSON) y diagnostics. `RequestSummary`: name/method.
  `RequestUpdate`: collection/path/request. `RequestInspection.changed` es nullable;
  una revisión ausente no se interpreta como documento intacto.
- `FieldEdit { path: string[], value }`: índices como segmentos string;
  null elimina el nodo, no asigna un null YAML. `SaveInput { revision, edits }`
  nunca acepta un reemplazo de archivo. Hasta 1.024 ediciones, rutas permitidas
  en `Document::edited`; revisión vencida/otra raíz/cambio externo bloquean guardar.
- `SendInput`: path, environment nullable, method, url, baseUrl; revision opcional,
  edits (default []), secrets (default {}) y document opcional. Request guardado:
  carga vigente + borrador por revisión; document presente: request transitorio,
  sin revisión de disco, con defaults de colección si está activa. Los cambios
  de ejecución no se guardan; baseUrl es override transitorio del mismo nombre.
- `CreateRequestInput`: name; fileName, method (GET por defecto), url, folder,
  edits y document opcionales. `RenameRequestInput`: revision/name/fileName.
  Los DTOs de entrada anteriores rechazan campos desconocidos en Rust.
- `ResponseMeta`: id/status/headers[{name,value}]/bodyBytes/elapsedMs/text/historyWarning.
  `text` comprueba UTF-8 del cuerpo completo. Headers IPC son texto con reemplazo
  ante bytes inválidos; para bytes originales de headers usar el core o CLI.
- `VariablePreview`: name/value nullable/scope/path/secret. El valor de secretos
  declarados no se expone. `HistoryView`: entries y warning; entradas de timestamp,
  requestPath relativo, method/status/outcome/elapsedMs/bodyBytes. No payloads.

## Respuesta, concurrencia y cancelación

Una ejecución activa por Session; otra operación incompatible falla con string
(`Another operation is running` / `A request is already running`), no se encola
silenciosamente. `cancel_request` señala el token activo y no toma el bloqueo de
operación; es inocuo si no hay envío. No revierte efectos HTTP ya realizados.
Los errores de Session/core se rechazan como **strings**, sin códigos JSON
estructurados estables. Fallos de history se devuelven como warning sin convertir
una respuesta HTTP correcta en fallo. Cambiar esto requerirá actualizar ambos lados.

Rust conserva una última respuesta de hasta 16 MiB. Un nuevo envío la invalida;
el id anterior ya no se puede leer. `read_response` usa offset en **bytes**, admite
offset igual al tamaño (base64 vacío) y rechaza offset mayor o id vencido.
La UI decodifica base64, avanza por bytes y conserva TextDecoder por respuesta
para UTF-8 cortado entre chunks. No es descarga progresiva por red ni streaming
a archivo. `release_response` solo libera el id actual coincidente; no cancela
otro envío ni elimina una respuesta posterior. Las tabs pueden conservar texto
ya cargado en frontend aunque la respuesta Rust anterior haya caducado.

## Persistencia, eventos y seguridad

El evento `collection-changed` contiene roots y warning. Watcher, refresco manual
y foco conducen a `refresh_collections`/`inspect_request`; un borrador editado
no se sobrescribe automáticamente. Cambiar colección/workspace invalida revisiones;
la UI resuelve guardar/descartar antes de la transición. Quitar referencias no
borra carpetas; crear/eliminar/renombrar sí son operaciones explícitas.

[RecoverySnapshot](../../apps/desktop/src-tauri/src/recovery.rs) v1 contiene
workspaceId/activeTab/home/tabs y configuración/edits/source; máximo 128 tabs y
16 MiB del respaldo. RecoveryView contiene snapshot y warning. Se excluyen
respuestas y secretos runtime en el productor
[recovery.ts](../../apps/desktop/src/lib/features/requests/recovery.ts);
`value/source` son configuración, no un canal con redacción automática de literales.
Registro, recovery e history van a datos locales, fuera de la colección.

Los valores enviados en `secrets` se usan solo para ejecutar; no entran en
save_document, history ni recovery de la UI. Respuestas, document y literales
sí pueden ser sensibles. No añadir logs de argumentos/respuestas ni ampliar
permisos por introducir DTOs nuevos. Pruebas actuales:
[Session](../../apps/desktop/src-tauri/src/session/tests.rs),
[recovery](../../apps/desktop/src-tauri/src/recovery/tests.rs) y
[frontend](../../apps/desktop/tests/unit/recovery.test.mjs).
