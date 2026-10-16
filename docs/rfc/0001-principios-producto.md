# RFC-0001 — Principios de producto y expansión

Estado: decisiones de NP-016, 2026-10-16. Aplican a los contratos actuales y a
las ampliaciones futuras; no declaran implementada toda la visión.

## Archivos y ejecución local

OpenCollection en disco es la fuente de verdad. No exigir cuenta, cloud,
servidor local permanente ni base propietaria para reconstruir una colección.
Abrir/validar no escribe, no importa automáticamente ni ejecuta scripts, Git o
plugins. Las rutas autorizadas delimitan los accesos; elegir una colección no
autoriza modificar el resto de un repositorio Git que la contenga.

El [core](../../crates/core/src/lib.rs) comparte carga, preparación HTTP y storage
entre Desktop y CLI. Los adaptadores coordinan interacción, transporte de DTOs
y salida; no implementan otra semántica de auth, variables o validación.

## Compatibilidad y conservación

El [RFC-0003](0003-contrato-opencollection.md) manda sobre formato y guardado.
Leer, conservar, editar y ejecutar son capacidades distintas. Un campo ajeno
no se elimina para hacer pasar una validación; configuración activa no soportada
produce un diagnóstico antes de red. Guardar es explícito y trabaja sobre
plantillas, nunca sobre el request interpolado. Cambios externos bloquean
overwrite; un borrador se conserva hasta guardar, descartar o copiar.

OpenCollection 1.0.0 y el esquema fijado 0.14.0 no cambian en NP-016. No introducir
IDs obligatorios, layout nuevo ni migraciones al abrir. La atomicidad actual es
por archivo y conserva una ventana residual ante escritores externos.

## Privacidad y control

Secretos suministrados en memoria quedan fuera de YAML, history, recovery,
diagnósticos y logs. Esto no redacta credenciales literales ya escritas en la
configuración ni contenido sensible devuelto por el servidor: la respuesta
Desktop y stdout de la CLI pueden contenerlos. History solo representa metadatos;
recovery conserva configuración/bodies del borrador, sin respuestas/runtime.

Toda ampliación define entradas, salidas, errores, límites, cancelación,
retención y borrado. No ejecutar texto importado, descargar refs remotas o
activar helpers por abrir datos. Cancelar detiene trabajo futuro; no promete
deshacer efectos HTTP, un commit ya creado ni archivos ya publicados.

## Decisiones para avanzar

Implementar Git/imports según [sus contratos](README.md), sin anticipar crates,
traits o servicios antes de necesitarlos. Los tests prueban comportamiento real
y casos adversos; un RFC escrito no sustituye evidencia de implementación.
NP-010/012 y los demás gates de C0 siguen pendientes. NP-013 tiene aceptación
ALPHA Linux con concesión de memoria; no equivale a ausencia de fugas, cobertura
Windows/macOS ni cumplimiento de la meta global de RAM.
