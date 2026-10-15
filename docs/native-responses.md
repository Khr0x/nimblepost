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

## Carga y fronteras

`responses-linux-v1`: cinco lanzamientos por serie, tres ciclos por lanzamiento.
Cada ciclo abre 64 pestañas y carga cuatro textos completos de 2 MiB. Después
reemplaza la última respuesta con JSON válido de exactamente 10 MiB, y luego con
una línea Unicode de 2 MiB. Lee fragmentos IPC reales de 64 KiB, comprueba bytes y
SHA-256, cambio de tab y copia. El host inyecta un click X11 real sobre Copy y
calcula SHA-256 de la selección leída por xclip; no intercepta ni simula clipboard.

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
Una comparación passed del mismo binario es validación acotada del protocolo/carga,
no una mejora de rendimiento entre versiones.

La aceptación y los resultados observados se mantienen en Obsidian, task NP-013.
