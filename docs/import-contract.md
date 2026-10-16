# Contrato objetivo de importaciones

Estado: decisión de diseño NP-016, 2026-10-16. **No hay importadores implementados.**
Consumidores: NP-019 cURL, NP-020 Postman, NP-021 OpenAPI. El gate C0 → M1
permanece. [Principios](rfc/0001-principios-producto.md) y
[RFC-0003](rfc/0003-contrato-opencollection.md) definen privacidad/formato destino.

## Flujo y resultado comunes

1. El usuario elige texto/archivos locales y formato. Parsear como datos, sin
   shell, requests HTTP, scripts ni escritura del origen/destino.
2. Generar en memoria un plan revisable: requests/carpetas/environments candidatos,
   paths destino propuestos, procedencia y diagnósticos de conversión por elemento.
3. Mostrar lo representable, las pérdidas y los bloqueos. Cada diagnóstico tiene
   categoría, ubicación de origen, severidad, explicación sin valores sensibles
   y acción posible. Clasificar elementos como convertible, requiere decisión
   o bloqueado; no anunciar conversión sin pérdidas ante omisiones.
4. Elegir destino y selección; resolver credenciales, colisiones y omisiones
   explícitas. No escribir por abrir/cerrar preview ni al cancelar.
5. Validar el plan completo con formato/subconjunto actuales y persistir solo lo
   confirmado; refrescar el árbol. El envío posterior sigue siendo otra acción.

El resultado lógico es plan + diagnósticos + conteos; no fija todavía DTOs ni
nombres IPC/CLI. La futura implementación debe mantener la lógica de conversión
compartida en Rust y adaptadores pequeños; sin crear ahora un motor de plugins.
El origen se conserva intacto y no se copia automáticamente a reportes persistidos.

## Subconjuntos decididos para las primeras tareas

Estas son decisiones de NimblePost, no promesas de compatibilidad completa con
las herramientas de origen. Cada tarea fijará versiones/esquemas y fixtures de
procedencia antes de implementar su parser; una ampliación exige refinar su spec.

| Tarea | Primera entrada y mapeo | Bloqueos/omisiones visibles |
|---|---|---|
| NP-019 | Un comando cURL textual; URL HTTP/S, método, headers repetidos y body literal text/JSON/XML; tokenización con quoting definido y probado | No ejecutar cURL; sin pipelines, redirecciones de shell, sustituciones, expansiones, `@archivo`, config externa, multipart, proxy, certificados o flags desconocidos activos. No interpretar esas construcciones parcialmente como un request válido |
| NP-020 | Archivo local collection Postman v2.1 y environment seleccionado; jerarquía, método/URL, headers/query duplicados, variables string, raw body y Basic/Bearer/API Key representables | Scripts/tests/dynamic variables, otros bodies/auth o settings no soportados requieren diagnóstico; auth heredada se resuelve sin convertirla silenciosamente en ausencia de auth |
| NP-021 | Documento local OpenAPI 3.0.x/3.1.x; operaciones HTTP seleccionadas, server explícito, ejemplos y parámetros representables, body text/JSON/XML | Sin inventar payloads requeridos ni credenciales. Parámetros de ruta requieren valor de ejemplo/decisión y sustitución antes de generar URL; no producir path params activos que el core rechaza. Serializaciones/security no representables bloquean el elemento |

Orden/duplicados y scopes no se aplanan con un mapa si cambia la semántica.
Un script/auth/settings activos incompatibles no pueden perderse para que el
request parezca ejecutable: bloquear ese elemento. Solo un descarte explícito,
visible en preview y en el informe de pérdidas puede producir una variante
reducida; no permitir descartes silenciosos. Documentación no ejecutable puede
omitirse con warning. Campos ajenos ya presentes en el **destino** no se eliminan.

## Secretos y referencias

Credenciales literales conocidas (auth, headers de autorización y campos
marcados secretos) se ocultan en preview/diagnósticos y no se escriben por defecto.
Solicitar quitar, convertir en placeholder con declaración de secreto sin valor,
o persistir el literal solo mediante decisión explícita y visible. Ningún valor
del mapa runtime de ejecución se exporta, incluso con esa decisión. No se puede
detectar todo secreto en nombres arbitrarios, URL o body; el preview debe advertir
la revisión de esos datos. No incluir el texto original en recovery ni reportes.
Un plan importado no se incorpora al recovery normal hasta resolver secretos.

Refs OpenAPI: internas y archivos locales bajo una raíz elegida expresamente;
sin red, `file:` arbitrario ni escape por symlinks/`..`. Detectar ciclos, refs
ausentes y estructura ambigua; no resolverlos con descargas o expansión infinita.
Resolver modelos no ejecuta ejemplos. La semántica concreta de quoting, refs y
serialización se acreditará con fixtures adversos en NP-019/020/021.

## Límites y persistencia

Presupuestos iniciales comunes: 8 MiB por entrada/archivo generado, 32 MiB
agregados de entrada/refs y de salida generada, profundidad de parseo/refs 64,
hasta 1.000 archivos de origen/refs y 10.000 requests candidatos. Rechazar
ambigüedad, claves duplicadas y YAML con aliases/anchors/tags; validar límites
antes de expandir árboles/refs. El texto cURL también tiene límite de 8 MiB.
Al exceder, error explícito y cero publicación; no recortar una conversión.

Primer corte: cURL entrega borrador; Postman/OpenAPI crean una colección nueva
en directorio todavía inexistente bajo un padre elegido. Colisiones bloquean,
sin sobrescribir, fusionar ni reutilizar directorios preexistentes. Validar
paths/bytes y preparar fuera de la colección activa; crear destino sin reemplazo
y registrar cada archivo propio. Ante fallo/cancelación limpiar solo artefactos
propios que sigan intactos. Si otro proceso los modificó, preservarlos e informar
estado parcial/ruta de recuperación; no prometer transacción multiarchivo ni
borrar datos ajenos. Antes de un futuro merge en colección existente se necesita
otro contrato de conflictos/revisiones y aceptación; no aprovechar overwrite.

Reutilizar validadores `Document` y reglas de creación/guardado del
[core](../crates/core/src/lib.rs), sin serializar un request interpolado.
No cambiar OpenCollection 1.0.0 ni introducir manifiesto propietario. Lo que no
pueda guardarse bajo los límites actuales necesita otra tarea, no una excepción
silenciosa del importador.

## Errores, cancelación y aceptación futura

Categorías objetivo: invalid-source, unsupported-format/version/feature,
ambiguous, missing/cyclic-reference, invalid-path, limit, secret-decision,
destination-exists/changed, cancelled y io. Son categorías del diseño, no nuevas
variantes implementadas del Error HTTP. Diagnósticos y reporte opcional solo
describen conteos/ubicaciones/decisiones; no valores de auth/body ni texto fuente.

Parseo/conversión comprueban cancelación entre unidades y refs; cancelar en
preview no escribe. Durante publicación se aplica la limpieza conservadora
anterior y se informa lo realmente publicado; no declarar rollback completo sin
comprobarlo. Sin timeout de red porque las importaciones no hacen red.

Cada tarea necesita fixtures positivos/adversos: quoting y shell hostil (cURL),
jerarquía/herencia/scripts/secretos (Postman), refs/ciclos/parámetros/security
(OpenAPI); además límites, claves duplicadas, colisiones, cancelación, archivos
externos, pérdida explícita y cero secretos en reportes/recovery. Round-trip
destino → `Document` → `prepare` para casos ejecutables contra loopback, sin
afirmar round-trip perfecto al formato de origen. NP-016 no ejecuta estas
pruebas de importadores que todavía no existen.
