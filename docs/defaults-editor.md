# Defaults de colección y carpeta

NP-014 añade **Edit defaults** al menú de colección/carpeta. El diálogo permite
editar `request.headers`, `request.auth` y `request.variables` del scope elegido.
Los metadatos, scripts, settings y campos ajenos se conservan; no se editan aquí.
Auth admite Inherit, Basic, Bearer y API Key; otras variantes permanecen retenidas.
Valores de configuración se guardan literalmente: usar placeholders para credenciales.

## Guardar y salir

Abrir/inspeccionar no escribe. **Save** (Cmd/Ctrl+S) aplica FieldEdit sobre el
documento original y su revisión; **Discard and reload** descarta el borrador y
relee el archivo/ancestros. Cerrar/Escape con cambios ofrece Keep editing,
Discard and close o Save and close. Un fallo conserva el borrador. La ventana
no cierra normalmente con cambios de defaults sin resolver; este borrador no
forma parte del recovery de requests y no promete recuperación tras cierre forzado.

Una carpeta sin `folder.yml` abre un documento transitorio, revisión 0. Save
crea exclusivamente ese archivo, sin crear directorios ni sobrescribir un archivo
que haya aparecido mientras se editaba. No hay creación de colección desde esta
operación. Archivos existentes modificados o eliminados externamente generan
conflicto; no se recrean por Save. Reload es una decisión explícita de descarte.
Colecciones readonly permiten inspección y bloquean escritura también en Rust.

El guardado preserva comentarios, campos ajenos, permisos y no-op/mtime usando
el storage común. Mantiene la ventana residual comprobación final → rename del
[RFC-0003](rfc/0003-contrato-opencollection.md); no hay transacción multiarchivo.
YAML ambiguo, formato/layout incompatible o escape de raíz se rechazan. Datos
no editables y diagnósticos de esquema no se borran para hacer pasar validación.

## Procedencia

**Sources** enumera archivos desde colección hasta carpeta cercana y marca
campos local/inherited/overridden/disabled/suppressed. Headers se agrupan sin
distinguir mayúsculas: un grupo local totalmente deshabilitado suprime el
heredado. Variables distinguen mayúsculas; una fila deshabilitada deja buscar
el nombre en scopes inferiores. Auth inherit continúa hacia el ancestro.
Esta vista presenta nombres/procedencia, no valores interpolados ni otra
implementación de la semántica de ejecución. Environment/request/runtime pueden
sobrescribir estos defaults según el contrato del motor.

Ancestros se consultan al abrir/Reload; no son un snapshot transaccional de varios
archivos. Un envío siempre relee defaults actuales. Save actualiza el contexto
de variables de requests abiertos y conserva sus borradores y respuestas.

## Frontera técnica

- [load_defaults](../crates/core/src/collection.rs): lectura de colección, ancestros
  y carpeta objetivo; raíz canónica, rutas relativas internas, profundidad ≤64,
  sin `.git`, `node_modules`, `environments` o symlinks fuera de raíz.
- [Document::edited / create_folder_defaults](../crates/core/src/storage.rs):
  whitelist del scope, máximo 1.024 ediciones, CST y creación exclusiva validada.
- [Session](../apps/desktop/src-tauri/src/session.rs): `read_defaults` devuelve
  `{path, document, ancestors, missing}`; ancestros solo llevan nombres/estado
  de headers/variables y tipo de auth, sin valores. `create_folder_defaults`
  recibe folder/edits; existente usa `save_document` sin cambiar su DTO.
- [DefaultsEditor](../apps/desktop/src/lib/features/collections/DefaultsEditor.svelte):
  diálogo lazy; [AuthFields](../apps/desktop/src/lib/components/AuthFields.svelte)
  compartido con RequestEditor. No nuevos permisos o dependencias.

No se aceptan secretos runtime como argumentos de estas operaciones ni se
incorporan a YAML/history/recovery/logs. Configuración literal ya existente puede
contener credenciales; no hay redacción universal. Sources nunca transporta
valores de ancestros y el editor no ejecuta requests al abrir/guardar.

Tests: [storage](../crates/core/tests/storage.rs),
[Session](../apps/desktop/src-tauri/src/session/tests.rs) y
[procedencia](../apps/desktop/tests/unit/defaults.test.mjs). El smoke nativo de
NP-014 usa datos/perfil temporales; no reemplaza el checklist x64 NP-010 ni
ejecuta campañas de rendimiento NP-013.
