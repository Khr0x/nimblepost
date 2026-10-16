# Respuestas grandes, retención e interacción — NP-013

La aceptación ALPHA cubre la app instalada **Linux x64**, release con
`native-validation` y `VITE_NATIVE_VALIDATION=1`, perfil/WebView efímeros y sin
inspector ni GC forzado. No acredita Windows/macOS ni objetivos de NP-012.
El workflow pesado sigue exclusivamente manual; el CI ligero no cambia.

## Contrato ALPHA v2 y ejecución acotada

Por decisión explícita del usuario, `responses-linux-v2` sustituye la aceptación
v1 de dos series de cinco lanzamientos y p95 local de 100 ms. Los reportes v1
conservan sus fallos e identidad; no se reclasifican ni se aceptan como v2.
100 ms queda como meta de optimización, no como gate de todas las acciones.

En **Actions → Native performance tooling → Run workflow**, elegir la rama,
`scenario: responses` y `launches: 3` (valores por defecto). Construye e instala
Debian instrumentado y ejecuta **una sola serie de tres lanzamientos**, con los
tres ciclos completos de cada uno. Guarda `acceptance.json` y recalcula sus
presupuestos en `acceptance-evaluation.json`. No necesita referencia/candidata.
`launches: 1` es diagnóstico sin aceptación. Startup mantiene su contrato de
cinco lanzamientos: usar explícitamente `scenario: startup, launches: 5`.

Ejecutar la aceptación al cerrar el hito o investigar una regresión, no por cada
cambio cosmético. Esta última validación de NP-013 se ejecuta una vez: si falla,
conservar evidencia y decidir el siguiente paso sin repetir hasta que salga verde.
No ejecutar perfiles de heap/asignador ni ablaciones como parte de aceptación.

Para ejecución local con requisitos Tauri, Xvfb, dbus, xclip y xdotool:

```sh
npm ci
export NIMBLEPOST_BUILD_SHA=$(git rev-parse HEAD)
VITE_NATIVE_VALIDATION=1 npm run tauri --workspace @nimblepost/desktop -- build --ci --features native-validation -- --locked
sudo apt-get install -y ./target/release/bundle/deb/*.deb
dbus-run-session -- xvfb-run -a node scripts/native-responses.mjs run \
  --binary /usr/bin/NimblePost --launches 3 \
  --conditions 'describir display, alimentación, modo energético y carga concurrente' \
  --out reports/acceptance.json
node scripts/native-responses.mjs evaluate \
  --report reports/acceptance.json --out reports/acceptance-evaluation.json
```

No publicar el instalador instrumentado. Producción omite feature/variables y
módulos de validación; el workflow comprueba esa exclusión. El runner exige
fuentes tracked limpias y SHA compilado coincidente. No modifica HOME ni usa
perfiles/colecciones reales. Xvfb es desechable; el clipboard usa datos sintéticos.

## Carga y fronteras que se conservan

Por lanzamiento hay tres ciclos y 28 fases. Cada ciclo abre 64 pestañas y carga
cuatro textos completos de 2 MiB. Reemplaza la última respuesta con JSON válido
de exactamente 10 MiB, después con una línea Unicode de 2 MiB. Lee fragmentos
IPC reales de 64 KiB y comprueba bytes/SHA-256, cambio de tab y copia. El host
inyecta un click X11 real sobre Copy y calcula SHA-256 de la selección de xclip.
La línea parte un emoji en el primer límite de 64 KiB; cambia de tab con el cuerpo
parcial y exige integridad al completar. El decoder no entra en recuperación.

HTTP loopback sirve 17 MiB chunked sin Content-Length: exige error explícito por
límite de 16 MiB y ningún cuerpo parcial retenido. Cancela una respuesta lenta
con el botón real y demuestra tráfico positivo/cierre antes del final; un envío
posterior funciona. Copy conserva el cuerpo original completo. El receptor limita
también la capacidad a 16 MiB; v2 exige `backendMemory.responseCapacity`, junto
con bytes usados y ceros al cerrar/reset.

La sonda mide desde invocar la acción hasta el estado esperado y dos frames.
Describe latencia instrumentada, sin certificar el final del pintado GPU ni la
experiencia en una workstation. Conservar handler/flush/frames y outliers.

| Tipo de acción | p95 ALPHA | Máximo |
|---|---:|---:|
| Fragmento (`chunk`) y scroll | ≤150 ms | ≤250 ms |
| Cambio de pestaña, incluida respuesta parcial | ≤250 ms | ≤250 ms |
| Wrap | ≤100 ms | ≤250 ms |
| Cancelación durante transferencia | descriptivo | ≤1.000 ms |

El p95 se calcula por tipo sobre la serie, nunca agregado entre tipos. Con nueve
muestras de pestaña por tipo el p95 coincide con el máximo y es descriptivo.
Se publican tamaños de muestra y mínimos/medianas/máximos. Un fallo funcional,
de privacidad o de presupuesto impide aceptar la serie.

Al cerrar hay cero cuerpos frontend/backend; al vaciar workspace, cero snapshots,
etiquetas, intentos y cachés. El secreto sintético declarado solo vive en memoria;
el servidor registra si lo recibió. Se verifica YAML intacto y ausencia de cuerpos
/secreto en history/recovery/perfil. Los reportes contienen hashes/contadores.

## Memoria y evaluación de una sola serie

Familia Linux por /proc, PID/starttime y helpers WebKit obligatorios. RSS/PSS se
leen cada ~100 ms durante uso. El pico es **máximo observado**; las lecturas de
procesos no son atómicas. RSS y PSS no son intercambiables y el muestreo añade carga.

En cada fase asentada se esperan tres segundos y se guardan seis muestras
separadas al menos 200 ms. En CADA lanzamiento, el incremento de mediana PSS
entre reset de ciclo 2 y 3 debe ser ≤max(16 MiB, 5% del reset del ciclo 2).
Este presupuesto no cambia. No acredita 80 MB ni ausencia general de fugas.
No hay banda calibrada contra otra serie; se evalúan función, límites de
interacción y estabilización en la misma serie estándar de tres lanzamientos.

`evaluate` recalcula desde raw, sin confiar en summaries. Rechaza v1, reportes
parciales, running/failed/diagnostic y perfiles. `status: passed` del raw indica
serie funcional completa; `summary.budgetStatus` y el archivo de evaluación
indican aceptación de los presupuestos ALPHA. `absoluteTargetsEvaluated: false`
se refiere a los objetivos globales de NP-012, no a estos rangos locales.
CLI: 0 pasó, 2 presupuesto incumplido, 1 datos/función inválidos. Fallos de memoria
aparecen por lanzamiento, sin ocultarlos entre medianas globales. El pipeline
propaga salidas no cero y conserva artefactos incluso ante un fallo.

El JSON se guarda atómicamente por fase/lanzamiento. El job tiene límite de
60 minutos; tres lanzamientos reducen la campaña obligatoria de diez a tres,
conservando los escenarios funcionales. La aceptación y sus límites viven también
en Obsidian, tarea NP-013. Los diagnósticos siguientes se mantienen disponibles
para investigaciones futuras y no forman parte de esta última serie.

## Diagnosticar memoria y latencia

El escenario manual `response-diagnostics` fuerza un solo lanzamiento y utiliza
`responses-linux-diagnostic-v2`. Conserva los fixtures y las comprobaciones de
integridad, copia, cancelación y privacidad. Nunca genera aceptación ni puede
compararse como referencia. No se restan sus controles a los tiempos observados.

Por fase añade cinco controles sin acción: timer, dos frames y lectura IPC de
recovery; visibilidad/foco y presión CPU del host. Por acción conserva spans IPC
(comando, inicio y duración; sin argumentos), además de handler/flush/frames.
Los comandos concurrentes pueden solaparse: sus duraciones no se suman como CPU.
Conserva tiempos backend de request, entorno, variables, chunks y recovery en un
buffer limitado a 1.024 entradas, drenado por fase; cualquier descarte hace fallar
el diagnóstico. Tras los tres ciclos espera 30 s sin acciones ni GC y añade la
fase `idle-reset` (29 fases en total), para observar reclamación natural.

Compila aparte `scripts/response-allocator.c`, sin incluirlo en el paquete. Un
preload habilitado solo para la familia NimblePost/WebKit del perfil temporal
consulta `malloc_info` cada segundo y exporta XML de contadores. El host conserva
XML, PID/starttime, edad de muestra y tamaños de arenas/libres/mmap por fase,
además de PSS, anónima y CPU. Si falta el contador del proceso principal falla;
los helpers que no hereden el preload quedan sin contador, explícitamente.

`system-current - free` estima ocupación de arenas: incluye metadatos/tcache y
no identifica objetos vivos. mmap de glibc se registra aparte; asignadores
propios y mapeos directos de WebKit pueden quedar fuera. PSS y tamaños de arenas
no son equivalentes. El preload añade un hilo y carga; no demuestra ausencia de
fugas ni acredita rendimiento de producción. No fuerza GC, trim ni modifica
límites del asignador. Véase [malloc_info](https://man7.org/linux/man-pages/man3/malloc_info.3.html)
y las limitaciones de [mallinfo](https://man7.org/linux/man-pages/man3/mallinfo.3.html).

### Perfil fino antes de corregir

El escenario manual `response-profile` usa `responses-linux-profile-v3`: un ciclo,
11 fases y reposo final. Activa agregados frontend acotados (32 etapas/16 muestras
por etapa), mapeos por familia y heaptrack sobre el WebKit del perfil sintético.
Abre/carga las 64 tabs antes de insertar heaptrack; comprueba PID/starttime e
inyección antes de continuar con JSON/línea/cierre. Omite el sampler malloc_info
durante la inyección para separar herramientas; conserva PSS/mapeos/CPU. Falla explícitamente
si no se puede perfilar. GDB requiere ptrace en el runner efímero; ningún otro
escenario modifica esa configuración. Heaptrack puede alterar tiempos/uso y su
inserción es inestable: este perfil no acredita rendimiento de producción.

En la fase de línea compara tres pares de tabs/scroll por modo: baseline,
sin transiciones, fuente del sistema, contención layout/paint, tabs ocultos y
baseline final. Los estilos experimentales se retiran; no cambian producción.
Los registros permiten localizar trabajo de índices, geometría/ResizeObserver,
lecturas de layout y recovery. Presupuestos y protocolo de aceptación no cambian.

Conserva resumen de callers/picos, timeline Massif y digest/tamaño del trace.
El trace temporal se elimina con el perfil; no se capturan cuerpos de objetos.
La captura empieza tras arrancar: no ve asignaciones previas ni todos los
asignadores propios, y algunos símbolos pueden faltar. `Not freed` al terminar
el proceso significa retención observada, no prueba de fuga. Ver
[heaptrack](https://github.com/KDE/heaptrack#using-heaptrack).
El plazo es ocho minutos por lanzamiento; solicita profundidad de 16 frames por
entorno (según soporte de la versión instalada). Ante fallo conserva el resumen
que alcance a producir y un error explícito. La v1 anterior falló por timeout y
no produjo stacks utilizables; su evidencia permanece separada.

`response-ui-profile` repite el mismo ciclo/ablaciones/mapeos y timings sin
heaptrack, con protocolo `responses-linux-ui-profile-v2`. Es el control para
localizar coste UI sin inyección; nunca acredita aceptación ni se mezcla con el
perfil intrusivo. Las diferencias entre modos dentro de una corrida orientan
el siguiente cambio; no comparar porcentajes entre jobs/hosts diferentes.

La barra conserva todos los tabs en DOM y teclado nativo. Un fieldset agrupa el
estado disabled, en vez de actualizar 128 atributos. Los botones de la app conservan
transiciones de color/borde/sombra/transform, sin animar opacidad al alternar loading.
La sonda verifica bloqueo nativo de click/foco y visibilidad del tab activo.

El visor calcula el anchor geométrico solo cuando lo restaura (cambio de wrap o
ancho con el mismo texto). Al añadir fragmentos evita una tabla Float64Array
transitoria completa; el perfil exige en JSON ≤un cálculo por índice más 12 para
asentamiento de viewport. Contenido, copia y tratamiento Unicode no cambian.

Los perfiles finos añaden controles de opacidad de botones y disabled del grupo
de tabs (72 acciones de ablación en total). Tras cerrar el intervalo de cada
acción, registran hasta 128 animaciones activas: categoría DOM y propiedades CSS,
sin valores/texto. `dropped` declara truncamiento. No altera el protocolo de
aceptación ni el diagnóstico de tres ciclos; son controles dentro del proceso.
