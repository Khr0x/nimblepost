# RFC-0010 — Presupuestos y aceptación de rendimiento

Estado: contrato definido; herramientas de medición y comparación retiradas. Los
objetivos absolutos y las mediciones indicadas como pendientes no están certificados.
Fecha: 2026-10-03.

Este RFC conserva el protocolo como referencia de diseño e historial de las
mediciones realizadas. CI comprueba los instaladores y el arranque nativo;
actualmente no publica mediciones ni compara regresiones de rendimiento.

## Propósito

Convertir las metas de la [propuesta](../../roadmap/api-client-local-first-open-source-propuesta.md#41-performance-budgets)
en resultados reproducibles. Desktop se evalúa como aplicación completa: proceso
Rust y procesos WebView atribuidos. Una mejora debe conservar recuperación de
borradores, contenido de respuestas, acceso a archivos y funcionamiento del IPC.

Hay dos evaluaciones independientes:

- **Objetivos absolutos:** cold start menor de 800 ms, memoria en reposo menor
  de 80 MB, CPU cercana a cero e instalador comprimido menor de 35 MB.
- **Regresiones:** variación del candidato respecto de una referencia elegida
  explícitamente, con el mismo equipo, entorno y protocolo.

Una comparación aprobada no acredita los objetivos absolutos. Las metas pendientes
permanecen abiertas en el roadmap. Cambiar una meta requiere actualizar este RFC
y justificarlo con evidencia; aumentar un límite para acomodar un resultado no
constituye una optimización.

## Escenarios y condiciones

Usar `release` con `native-validation`, sin inspector, muestreo de pilas ni GC
forzado. Los builds de producción deben omitir las sondas. Los perfiles y
datasets son temporales, sin datos del usuario, con HTTP servido en localhost.
Mantener el equipo desbloqueado, alimentación y modo energético constantes,
sin compilaciones ni otras suites durante la serie. Registrar versión del
WebView y esos ajustes junto a la evidencia cuando no estén capturados por el
runner. El mismo hostname identifica el host mediante SHA-256; no es una
identidad de hardware infalible.

| Escenario | Datos y comprobaciones |
|---|---|
| Vacío | Sin colección ni pestañas; IPC y ejemplo empaquetado funcionales. |
| Recuperación | 100 requests y un borrador recuperado, editable y vinculado a su archivo. |
| Colección grande | 5.000 requests; filas virtualizadas, etiquetas visibles completas y lectura acotada a las ventanas iniciales. |
| Uso de respuestas | 64 pestañas, cuatro respuestas de texto completas de 2 MiB, reemplazo, cierre y cambio de workspace. |
| Navegación repetida | Seis recorridos del árbol; cachés acotadas y vacías al reset. |
| Ampliación pendiente | JSON de 10 MiB, texto de una línea larga y rechazo HTTP real al exceder 16 MiB. |

El último escenario debe verificar tamaño y contenido recibido/copiado, carga por
fragmentos, errores explícitos por tamaño, cancelación y respuesta del editor
durante la carga. El límite de 16 MiB actual sigue vigente; respuestas de 100 MB
requieren antes almacenamiento en archivo/streaming y su propio presupuesto.

## Arranque

Registrar dos fronteras, con reloj monotónico del runner:

- `launchToReadyMs`: inicio del proceso hasta recuperar el editor, URL habilitada
  y dos frames. Se conserva para interpretar las mediciones existentes.
- `launchToProfileMs`: además espera las etiquetas visibles y otros dos frames.
  Esta frontera representa la app lista en la comparación v1.

La recepción de un marcador incluye latencia de IPC y del sondeo de archivos.
Dos frames no certifican que la GPU haya terminado de pintar. Las etapas internas
se solapan y sus duraciones no se suman como desglose del total.

**Arranque habitual:** cachés normales del sistema. Cinco lanzamientos por
escenario para desarrollo, treinta para evaluar una release; conservar primera
muestra, mediana, mínimo, máximo y p95 por rango más próximo
(`ceil(0,95 × n)`, índice de base uno). Con cinco muestras, p95 coincide con el
máximo y es descriptivo; con treinta sigue siendo una estimación del entorno.

**Cold start:** una primera ejecución por sesión después de reiniciar el equipo,
cinco sesiones independientes por escenario/equipo. Preparar fixtures y hash del
binario antes del reinicio, esperar un intervalo fijo tras iniciar sesión y lanzar
sin preleer el ejecutable. Registrar tiempos individuales, mediana y máximo;
evaluar la meta de 800 ms sobre esas muestras, con alcance limitado al equipo y
protocolo. Registrar las condiciones de caché; reiniciar no demuestra que todas
las cachés de almacenamiento estén vacías.

El runner retirado leía el binario antes de lanzarlo para comprobar las sondas y
calcular su hash. Sus resultados medían arranque habitual; el protocolo cold start
necesita un lanzador separado y permanece pendiente.

## Memoria, CPU y picos

Guardar bytes exactos; mostrar MiB (`2^20` bytes). Las metas originales usan MB
decimales (`10^6` bytes): 80 MB equivalen aproximadamente a 76,3 MiB.

| Plataforma | Métrica principal prevista | Métricas complementarias |
|---|---|---|
| macOS | `phys_footprint` por PID y suma de la familia atribuida. | RSS y máximo histórico de huella por proceso. |
| Linux | PSS de `/proc/<pid>/smaps_rollup`, sumado para la familia. | RSS y componentes privados/compartidos. |
| Windows | Memoria privada comprometida por proceso y suma de la familia. | Working set y, cuando esté disponible, working set privado. |

Estas métricas describen contabilidades diferentes y requieren referencias y
presupuestos propios. Memoria privada comprometida en Windows no equivale a RAM
residente. La suma de huellas macOS no es RAM única deduplicada del sistema. El
RSS permanece como señal auxiliar de regresión; no acredita la meta de 80 MB.

La meta de reposo se evalúa en el escenario vacío, con la app y todos sus helpers;
100/5.000 requests requieren también presupuestos propios. Medir inicialmente
el coste base del host WebView ayuda a decidir qué optimizaciones son viables,
sin descontar ese coste del consumo declarado de NimblePost.

El protocolo v1 conserva la ventana existente: tres segundos de espera después
de las etiquetas y seis muestras separadas por un segundo; se resume su mediana.
La CPU es el delta de tiempo de CPU durante ese intervalo, con un núcleo al 100%.
Un cero puede proceder de la resolución del contador. Ampliar el reposo a una
ventana de treinta segundos y añadir PSS/memoria privada crea un protocolo nuevo
y necesita una nueva referencia; esas mediciones aún están pendientes.

Las fases de uso conservan seis muestras RSS y, en macOS con
diagnósticos de memoria, una captura de huella posterior. Un máximo entre
capturas se etiqueta **máximo observado**. Los máximos históricos de procesos
ocurren en tiempos distintos: no se suman como pico simultáneo de la aplicación.
Los snapshots del inspector y `sample` son diagnósticos separados, excluidos de
los controles de regresión. El inspector puede forzar GC y alterar las reservas.

## Respuestas, retención e interacción

Durante el uso debe conservarse todo el contenido y seguir funcionando la copia,
el cambio de pestañas y la cancelación. Al cerrar todas las pestañas se exige
cero cuerpos en caché; al vaciar workspace, cero bytes de respuesta/snapshots
en backend, etiquetas, intentos y cachés de árboles. Se conservan las protecciones
de borradores. Fallar una comprobación funcional bloquea inmediatamente.

La memoria física después de cerrar puede conservar reservas del WebView. Su
estabilización se evalúa repitiendo el mismo ciclo, con pausas iguales, y
comparando las fases equivalentes con una referencia. La banda por plataforma
debe fijarse tras obtener series repetidas; las corridas individuales existentes
no acreditan estabilización ni un límite global de 200 MiB. Los controles de
regresión v1 solo evalúan arranque y RSS en reposo; las fases de uso y sus picos
siguen siendo evidencia descriptiva.

La latencia de interacción durante carga aún necesita una sonda desde la acción
hasta el estado visible esperado y un presupuesto medido. Un timeout funcional
o pocas filas DOM no acredita fluidez. Queda pendiente ese control.

## Comparación v1 y aceptación

La comparación v1 define la evaluación de reportes completos con al menos cinco
lanzamientos. Recalcula métricas desde muestras individuales; rechaza series
parciales, fallidas, de profiler o de distinto host, CPU, RAM, SO, arquitectura,
dataset, modo de WebView, escenario o definiciones de medición. La confirmación
debe usar el mismo binario candidato y una identidad de serie distinta de las
dos anteriores. Cada medición genera `seriesId`; reutilizar el mismo reporte no
cuenta como una repetición. Comparar versiones de binarios distintas
es válido; la referencia elegida debe ser una serie de un estado aceptado.

Controles iniciales sobre la **mediana de `launchToProfileMs` y
`familyIdleRssMiB`**, calibrables al medir la variabilidad:

| Resultado | Condición | Salida CLI |
|---|---|---|
| `passed` | Ambas variaciones son menores de 10%. | 0 |
| `warning` | Alguna variación alcanza 10%, sin un bloqueo de 15% confirmado. | 0 |
| `needs-confirmation` | Alguna variación alcanza 15% y falta otra serie candidata. | 3 |
| `blocked` | La misma métrica alcanza 15% en dos series candidatas respecto de la referencia. | 2 |
| Error de datos/compatibilidad | Comparación inválida. | 1 |

Las condiciones porcentuales son inclusivas. Conservar los outliers y las dos
series, incluso si la repetición no confirma la regresión. El p95/máximo se
publica para revisar colas, pero v1 no añade un bloqueo estadístico sobre ellos.
CPU cercana a cero requiere presupuesto propio; no se aplica un porcentaje de
regresión relativo a una referencia igual a cero.

`status: passed` en una medición nativa acredita completar la serie y sus
comprobaciones funcionales. El archivo de comparación tiene su propio `status`;
contiene explícitamente que los objetivos absolutos no están evaluados. El bloqueo
previsto requiere una referencia compatible y una comparación, sin inventar una
referencia para un runner nuevo.

## Evidencia actual y trabajo restante

La validación nativa registró mejoras del árbol,
paneles y respuestas, con arranque habitual de 341–393 ms en una serie macOS M4.
La referencia del protocolo v1
completó cinco lanzamientos por escenario el 2026-10-03, con medianas de
445/446/512 ms hasta editor y etiquetas para 0/100/5.000 requests.
Los reportes históricos se retiraron del repositorio; esas mediciones no acreditan
los objetivos absolutos. Los últimos experimentos
no redujeron consistentemente el pico y fueron retirados. La atribución del pico
transitorio de WebContent sigue pendiente.

- [x] Contrato de métricas, unidades, fronteras y estados de aceptación.
- [x] Resúmenes mediana/p95/rango y hasta treinta lanzamientos en el runner retirado.
- [x] Comparación explícita y confirmación de regresiones en la herramienta retirada.
- [ ] Cold start controlado y objetivo de 800 ms acreditado.
- [ ] Ventana de reposo de treinta segundos y adaptadores PSS/memoria privada.
- [ ] Objetivo de 80 MB, presupuestos por escenario y CPU acreditados.
- [ ] Series repetidas de respuestas/retención, casos ampliados e interacción.
- [ ] Resultados nativos Windows/Linux x64 del estado actual.
- [ ] Instalador de producción comprimido menor de 35 MB medido por plataforma.

Fuentes para la interpretación de memoria:
[Apple](https://developer.apple.com/videos/play/wwdc2021/10180/),
[Linux](https://docs.kernel.org/filesystems/proc.html) y
[Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex).
