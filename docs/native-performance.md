# Medición reproducible — NP-011

Tooling Node 24+ sin dependencias nuevas. El primer adaptador es **Linux**:
familia WebKit por parentesco `/proc`, PID y tiempo de creación para detectar
reutilización de PID. macOS/Windows no están soportados; en macOS los helpers XPC
dependen de launchd y medir solo Rust daría resultados incompletos.

## Protocolo

`startup-rss-linux-v2` mide arranque **habitual**, no cold start. Ready espera el
editor/borrador y dos frames; profile también etiquetas visibles y dos frames.
Las fronteras usan reloj monotónico del lanzador, incluyendo IPC y sondeo de
marcadores cada 10 ms. No equivalen a comprobar el final del pintado de GPU.

Fixtures versionados 0/100/5000 requests en perfiles temporales. Los escenarios
no vacíos recuperan una pestaña con un cambio de URL sin guardar. Se verifican
recuperación, virtualización/etiquetas, IPC, ejemplo y paneles diferidos. Sin
scripts ni colecciones del usuario. WebViews efímeros y backend aislado; este
modo es distinto de v1 y sus reportes **no son comparables**.

Cinco a treinta lanzamientos por escenario. Tras profile se esperan 3 s y se
toman seis muestras separadas al menos 1 s. RSS en bytes incluye Rust y todos
sus descendientes, con helpers WebKit obligatorios. RSS no es memoria física
única ni acredita 80 MB. CPU acumulativa por proceso es diagnóstica, sin gate.
No se activa inspector, muestreo de stacks ni GC. Se lee el binario para calcular
hash antes del lanzamiento: esto tampoco es cold start.

## Ejecución Linux

Con Node/Rust y requisitos nativos Tauri instalados, fuentes con commit:

```sh
npm ci
export NIMBLEPOST_BUILD_SHA=$(git rev-parse HEAD)
VITE_NATIVE_VALIDATION=1 npm run tauri --workspace @nimblepost/desktop -- build --ci --features native-validation -- --locked
sudo apt-get install -y ./target/release/bundle/deb/*.deb
dbus-run-session -- xvfb-run -a node scripts/native-performance.mjs run \
  --binary /usr/bin/NimblePost --launches 5 \
  --conditions 'describir display, alimentación, modo energético y carga concurrente' \
  --out reports/reference.json
```

El SHA compilado en la sonda debe coincidir con el checkout; no se aceptan
cambios tracked sin commit. Reporta hash binario, host hasheado, hardware, kernel,
WebKit y condiciones. Xvfb/GitHub-hosted no representa una workstation. El perfil
temporal se selecciona con NIMBLEPOST_VALIDATION_DIR; no cambiar HOME.

**No publicar este paquete instrumentado.** Producción omite las variables
VITE_NATIVE_VALIDATION/NIMBLEPOST_BUILD_SHA y la feature native-validation.
El workflow comprueba frontend ordinario sin eventos de validación. Las sondas
Rust se compilan solo con esa feature.

## Reportes y comparador

JSON schema/protocolo versionados, seriesId independiente, muestras individuales,
primera muestra/outliers, mediana/min/max y p95 por rango próximo. Se escribe
atómicamente tras cada lanzamiento. Series running/failed/parciales no son baseline.

```sh
node scripts/native-performance.mjs compare \
  --reference reports/reference.json --candidate reports/candidate.json \
  --accept-reference true --out reports/comparison.json
```

`--accept-reference true` selecciona explícitamente una referencia aceptada.
Para autocomprobar el tooling con el mismo binario usar `--tooling-check true`:
no evalúa gate de release. `--confirmation <reporte>` añade una tercera serie
independiente del mismo binario y SHA candidato. Reutilizar series o cambiar
host/hardware/WebKit/condiciones/protocolo/dataset/modo/número de lanzamientos
invalida la comparación. Las medianas se recalculan sin confiar en resúmenes.

Umbrales inclusivos RFC-0010 por escenario (profile y RSS idle): <10% passed,
≥10% warning, ≥15% sin repetición needs-confirmation, misma métrica ≥15% en dos
candidatos blocked. Exit codes: 0 passed/warning, 1 datos/uso inválido, 2 blocked,
3 needs-confirmation. Ningún resultado acredita objetivos absolutos.

## Workflow y pendientes

Se ejecuta exclusivamente a demanda: **Actions → Native performance tooling →
Run workflow → seleccionar rama → Run workflow**. GitHub ofrece el botón cuando
el workflow existe en la rama predeterminada. Registrar el SHA resuelto de la
ejecución. Push/PR conserva las pruebas rápidas del contrato/comparador con
`npm test`; las mediciones reales deben solicitarse al validar cambios de
rendimiento y conservar su evidencia para el SHA candidato.

`Native performance tooling` construye en Ubuntu x64 y recoge dos series del
mismo binario en el mismo host. Publica reportes completos/parciales durante 30
días; no publica instaladores instrumentados. La autocomprobación conserva
warning/needs-confirmation/blocked sin convertirlos en gate de producto; datos
inválidos sí fallan el job. No comparar runners efímeros distintos: para gates
entre versiones usar un host persistente y referencia seleccionada explícitamente.

NP-012: cold start, memoria física/PSS/private, CPU, tamaño y presupuestos absolutos.
NP-013: respuestas, retención e interacción. Adaptadores macOS/Windows requieren
atribución fiable y evidencia propia.

El escenario manual `responses` se describe en [NP-013](native-responses.md).
`startup` conserva el protocolo de arranque. Una corrida diagnóstica de un
lanzamiento de respuestas no equivale a una serie de aceptación.
