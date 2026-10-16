# RFC-0005 — Contrato objetivo del backend Git

Estado: decisión de diseño NP-016, 2026-10-16. **Backend no implementado.**
Consumidores futuros: NP-017 (status/diff/commit), NP-018 (ramas/remotos).
El gate C0 → M1 sigue aplicando aunque este contrato esté definido.

## Frontera y entradas/salidas

Elegir el Git del sistema como primer backend. No añadir ahora crate, trait,
daemon ni sustituto Git; implementar dentro de la estructura existente cuando
NP-017 lo requiera. Ejecutable ausente/incompatible produce diagnóstico, sin
instalación automática ni fallback con otra semántica.

| Operación objetivo | Entrada autorizada | Resultado esperado |
|---|---|---|
| Status | Colección abierta/raíz canónica | Raíz Git, HEAD opcional (repositorio sin commits), estado staged/unstaged/untracked por ruta y conflictos |
| Diff | Ruta seleccionada, lado index/worktree y revisión observada | Diff acotado o marca binario/no representable; no modifica índice |
| Stage/unstage | Lista explícita de rutas relativas y estado observado | Estado nuevo solo de esas rutas; ninguna selección implícita |
| Commit | Rutas seleccionadas, mensaje, HEAD/estado de índice esperado | OID y estado nuevo, o diagnóstico sin ocultar cambios |
| Checkout/pull/push (NP-018) | Rama/remoto/ref explícitos y estado esperado | Estado actualizado o conflictos visibles; nunca force/reset/stash automáticos |

Estos nombres describen responsabilidades, **no comandos IPC o tipos Rust ya
disponibles**. NP-017/018 concretarán sus DTOs antes de implementar. Los paths
no son URLs, código de shell ni argumentos libres de Git.

## Selección y conservación

La raíz Git puede ser ancestro de la colección. Mostrar esa relación y limitar
acciones a archivos bajo la colección seleccionada; rechazar paths fuera, `..`,
paths absolutos, metadata `.git` o escapes por symlinks. No stage/commit de
archivos hermanos por descubrir un repositorio más grande. Worktrees, `.git`
como archivo y submódulos necesitan resolución y fixtures explícitos; si no se
soportan en el primer corte, informar y bloquear mutación.

Status debe distinguir ambos lados del índice; nombres con espacios, tabs,
saltos de línea o prefijo `-` son datos. Usar argumentos separados sin shell,
separador de paths y salida delimitada sin depender de texto localizado.
Diff no invoca external-diff/textconv, pager ni editor. No ejecutar helpers,
filtros o comandos de configuración del repositorio implícitamente: si no se
puede garantizar un camino seguro, devolver configuración no soportada.

Antes de mutar, comprobar HEAD y estado relevante nuevamente; si cambiaron,
pedir refrescar/revisar. No prometer CAS frente a procesos Git externos. Respetar
locks de Git, sin borrarlos ni reintentar destructivamente. No tocar el índice
por una lectura de UI ni usar `add .`, `commit -a`, clean/reset o stash implícitos.

Commit solo incluye las rutas revisadas. Si existen staged ajenos a la selección,
bloquear commit y mostrar cómo resolverlos; no incluirlos ni hacer unstage
automático. Si una ruta tiene staged y unstaged, presentar esa diferencia y
confirmar qué contenido entra; no sustituir staging parcial sin decisión.
El primer corte no ejecuta hooks ni firma del repositorio: deshabilitarlos de
forma explícita y comunicar esa limitación; si una política exige firma/hooks,
informar no soportado en vez de fingir cumplimiento. No configurar identidad
ni credenciales globales desde NimblePost.

Un cambio de rama/pull con borradores o cambios que puedan perderse se bloquea
hasta resolución explícita. Reutilizar el flujo guardar/descartar/copia y
watcher/inspect de [Session](../../apps/desktop/src-tauri/src/session.rs),
[App.svelte](../../apps/desktop/src/App.svelte) y
[watcher](../../apps/desktop/src-tauri/src/watcher.rs). Tras mutación refrescar
árbol/revisiones; no reusar snapshots de otra rama. No implementar un segundo
almacén de borradores ni escribir requests interpolados para un commit.

## Límites, errores, cancelación y privacidad

Presupuestos iniciales de diseño para NP-017: status hasta 10.000 entradas,
salida hasta 2 MiB por operación y timeout 10 s de lectura/60 s de commit.
Al exceder, error `limit`/`timeout`, no truncado presentado como completo.
Cambiar estas cifras requiere actualizar el contrato antes del código.
NP-018 definirá el timeout de red y su cancelación antes de implementar.

Categorías objetivo: executable-unavailable, not-repository, unsupported,
invalid-path, stale-state, busy/lock, conflict, identity/auth, limit, timeout,
cancelled y process-failed. Diagnósticos propios y sanitizados; exit code o
stderr crudo de Git no son mensajes seguros para logs/UI persistida.
No añadir estas categorías al Error HTTP actual para simular implementación.

Operaciones largas admiten cancelación y espera de terminación/recolección del
proceso; contemplar descendientes y salidas acotadas. Cancelar no equivale a
rollback: volver a consultar HEAD/índice/worktree e informar si hubo commit o
cambio parcial, sin revertir archivos ajenos ni borrar locks.

Diffs, mensajes de commit, rutas y remotos pueden contener credenciales; son
datos de UI, no history/recovery/telemetría automática. No adjuntar valores de
`ExecutionContext.secrets` a argumentos, environment de Git ni commits.
NP-018 podrá usar credenciales del Git del sistema solo por una operación de
red explícita; debe definir prompts/helpers, redacción y cancelación propios.
Abrir una colección nunca dispara fetch/pull/push.

## Aceptación requerida en las tareas consumidoras

NP-017: repositorios temporales con/sin HEAD, staged/unstaged/untracked,
staging parcial, archivos ajenos ya staged, espacios/newlines/guiones, binarios,
raíz ancestro/escapes, cambios externos/locks, Git ausente, commit fallido y
cancelado; conservar hashes/índice/borradores. Verificar que hooks/helpers no se
ejecutan y ningún archivo sin selección entra al commit.
NP-018: remoto local, fast-forward/divergencia/conflicto, credenciales/error,
cancelación y borradores; nunca reset/force-push por resolver errores.
NP-016 acredita esta decisión documental, no esos escenarios aún sin backend.
