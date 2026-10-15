# Respuestas grandes, retención e interacción — NP-013

La validación inicial cubre la app instalada **Linux x64**, release con
`native-validation` y `VITE_NATIVE_VALIDATION=1`, perfil/WebView efímeros y sin
inspector ni GC forzado. No acredita Windows/macOS ni presupuestos absolutos de
NP-012. El workflow es exclusivamente manual.

## Ejecutar

En **Actions → Native performance tooling → Run workflow**, seleccionar la rama,
`scenario: responses` y `launches: 5`. Se construye/instala Debian instrumentado y
se ejecutan referencia y candidata en el mismo host. `launches: 1` ejecuta una sola serie diagnóstica:
no genera aceptación ni comparación. Conservar SHA resuelto, logs y artefactos.
Si la referencia incumple presupuestos, la corrida se detiene y conserva esos
resultados; no gasta otra serie ni calibra una banda desde una referencia rechazada.

Para ejecución local con requisitos Tauri, Xvfb, dbus, xclip y xdotool:

```sh
npm ci
export NIMBLEPOST_BUILD_SHA=$(git rev-parse HEAD)
VITE_NATIVE_VALIDATION=1 npm run tauri --workspace @nimblepost/desktop -- build --ci --features native-validation -- --locked
sudo apt-get install -y ./target/release/bundle/deb/*.deb
dbus-run-session -- xvfb-run -a node scripts/native-responses.mjs run \
  --binary /usr/bin/NimblePost --launches 5 \
  --conditions 'describir display, alimentación, modo energético y carga concurrente' \
  --out reports/reference.json
# Repetir con --out reports/candidate.json en el mismo entorno.
node scripts/native-responses.mjs compare \
  --reference reports/reference.json --candidate reports/candidate.json \
  --out reports/comparison.json
```

No publicar el instalador instrumentado. Las compilaciones ordinarias omiten
feature/variables y módulos de validación; el workflow comprueba esa exclusión.
El runner exige fuentes tracked limpias y SHA compilado coincidente. No modifica
HOME ni usa perfiles/colecciones reales. El display Xvfb es desechable; la prueba
reemplaza su clipboard con contenido sintético.

El receptor reserva Content-Length solo tras validar el límite, y mantiene también
la capacidad de crecimiento chunked dentro de 16 MiB. Los nuevos reportes incluyen
`backendMemory.responseCapacity`, además de bytes usados; el comparador comprueba
ese presupuesto cuando el campo está presente. Al cerrar/reset ambos son cero.

## Carga y fronteras

`responses-linux-v1`: cinco lanzamientos por serie, tres ciclos por lanzamiento.
Cada ciclo abre 64 pestañas y carga cuatro textos completos de 2 MiB. Después
reemplaza la última respuesta con JSON válido de exactamente 10 MiB, y luego con
una línea Unicode de 2 MiB. Lee fragmentos IPC reales de 64 KiB, comprueba bytes y
SHA-256, cambio de tab y copia. El host inyecta un click X11 real sobre Copy y
calcula SHA-256 de la selección leída por xclip; no intercepta ni simula clipboard.
La línea larga parte un emoji en el primer límite de 64 KiB; se cambia de tab
con la respuesta parcial y se exige integridad al completar. El decoder UTF-8
vive con su respuesta/pestaña y se descarta al cerrar; no entra en recuperación.

El servidor HTTP loopback también sirve 17 MiB chunked sin Content-Length:
se exige error explícito por límite de 16 MiB y ningún cuerpo parcial retenido.
Una respuesta lenta se cancela con el botón real de la UI; el registro del
servidor debe demostrar tráfico positivo y cierre antes de terminar. Un envío
posterior debe funcionar. Las respuestas grandes no se truncan: líneas largas
se muestran en segmentos visuales acotados y Copy conserva el original.

La sonda mide desde invocar la acción de UI hasta el estado esperado y dos frames
para carga de fragmento, scroll, wrap, cambio de tab y cancelación. Esto describe
latencia de la acción instrumentada, no de un dispositivo físico ni final del
pintado GPU. Presupuesto operativo fijado antes de medir: p95 por tipo de acción local ≤100 ms y
máximo ≤250 ms; cancelación durante transferencia ≤1000 ms. Los fallos se conservan.

Al cerrar: cero cuerpos frontend/backend. Al cambiar a workspace vacío: cero
snapshots, etiquetas, intentos y cachés. Un secreto sintético declarado se entrega
solo en memoria; el servidor registra únicamente si lo recibió. El runner verifica
YAML intacto y ausencia de cuerpos/secreto en history/recovery/perfil. Reportes
contienen hashes/contadores, nunca cuerpos ni el secreto.

## Memoria y aceptación

Familia Linux por /proc, PID/starttime y helpers WebKit obligatorios. RSS y PSS se
leen durante el uso cada ~100 ms. El máximo se etiqueta **máximo observado**:
se puede perder un pico entre capturas; lecturas de procesos no son atómicas.
PSS y RSS no son métricas intercambiables. El muestreo también añade carga.

En cada fase asentada se esperan 3 segundos y se guardan seis muestras separadas
al menos 200 ms. Al reset, el incremento de mediana PSS de ciclo 2 a 3 debe ser
≤max(16 MiB, 5% del reset del ciclo 2), en cada lanzamiento. Este es un presupuesto
operativo de esta carga y plataforma, sin acreditar 80 MB ni una ausencia general
de fugas. La referencia debe pasar función, interacción y estabilización. La
candidata también y su PSS reset máximo debe permanecer bajo el máximo de
referencia + max(16 MiB, rango de PSS reset observado en referencia). Banda calibrada
por host/carga/protocolo; no trasladarla a otras plataformas.

Comparación exige series diferentes, mismo host/hardware/SO/WebKit/condiciones,
fixtures y número de lanzamientos. Recalcula resultados desde muestras, sin confiar
en summaries. Estado de serie `passed` indica finalización funcional y de retención;
`summary.budgetStatus` evalúa interacción. `diagnostic`, `running` y `failed` no son
referencias válidas. CLI: 0 pasó, 2 presupuesto incumplido, 1 datos/función inválidos.
El JSON se guarda atómicamente al completar cada fase y lanzamiento; una corrida
interrumpida conserva su estado parcial y nunca se acepta como referencia.
El límite del job manual es 60 minutos: el diagnóstico de tres ciclos tarda
aproximadamente cinco minutos por lanzamiento en el runner observado.
Una comparación passed del mismo binario es validación acotada del protocolo/carga,
no una mejora de rendimiento entre versiones.

La aceptación y los resultados observados se mantienen en Obsidian, task NP-013.

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

El escenario manual `response-profile` usa `responses-linux-profile-v2`: un ciclo,
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
heaptrack, con protocolo `responses-linux-ui-profile-v1`. Es el control para
localizar coste UI sin inyección; nunca acredita aceptación ni se mezcla con el
perfil intrusivo. Las diferencias entre modos dentro de una corrida orientan
el siguiente cambio; no comparar porcentajes entre jobs/hosts diferentes.

La barra conserva todos los tabs en DOM y teclado nativo. Un fieldset agrupa el
estado disabled, en vez de actualizar 128 atributos; los tabs solo animan colores.
La sonda verifica bloqueo nativo de click/foco y visibilidad del tab activo.
