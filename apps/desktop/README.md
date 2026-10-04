# Desktop: Tauri 2 + Svelte 5

**Estado al 2026-10-01:** MVP 0 HTTP funcional en desarrollo ALPHA (`0.1.0`),
con workspaces locales, múltiples colecciones y edición en tabs.

El mapa de carpetas y sus responsabilidades está en
[RFC-0002 — Organización del repositorio](../../docs/rfc/0002-arquitectura-inicial.md#organización-del-repositorio--2026-10-01).
La UI se organiza en `src/lib/features`, el puente IPC en `src/lib/desktop`,
los componentes compartidos en `src/lib/components` y los estilos en `src/styles`.
Las pruebas locales están en `tests/unit`.

La app inicia en modo oscuro. El selector **Theme** permite cambiar entre Dark
y Light y conserva la preferencia localmente entre sesiones. La UI usa la paleta
**Zinc & Emerald**: fondo `#09090b`, tarjetas `#18181b`, bordes `#27272a`,
acento de marca `#10b981`, texto `#fafafa` y texto secundario `#a1a1aa`.
El esmeralda identifica las acciones primarias, el foco y los estados exitosos;
los métodos HTTP conservan sus colores funcionales y los errores usan rojo.
El tema claro adapta los grises Zinc y el contraste del esmeralda.
La ventana nativa sigue el tema elegido.

Los controles de las peticiones usan **shadcn-svelte**: Button, Input, Textarea,
Tabs, Select, Checkbox, Table, Breadcrumb, Dialog, Collapsible y ContextMenu.
Esto incluye Method, Body, Auth, environments, filas de Params/Headers/Variables,
creación y guardado, cierre de pestañas y JSON plegable. Los iconos usan **Lucide**
(`@lucide/svelte`). Los componentes viven en `src/lib/components/ui`, con Tailwind CSS 4.

Al añadir controles a las peticiones, reutiliza estos componentes o incorpora el
correspondiente desde shadcn; evita selects, checkboxes, botones y diálogos nativos
para esos controles. Conserva los tokens de tema y los colores de métodos HTTP.
Los selectores del sistema para abrir carpetas siguen siendo nativos.
Para añadir un componente desde el registro oficial:

```sh
cd apps/desktop
npx shadcn-svelte@1.7.0 add <componente>
```

Desde la raíz, con Node 24+/npm, Rust y los requisitos nativos de Tauri instalados:

```sh
npm ci
npm run desktop:dev
```

El ejecutable nativo se llama **NimblePost**, tanto en desarrollo como en el
bundle; el título de la ventana y el nombre de producto usan la misma marca.

En macOS se necesita Xcode/Command Line Tools. Para compilar el ejecutable sin
generar un instalador:

```sh
npm run desktop:build
```

El flujo desde cero es **New collection → nombre y carpeta → elegir ubicación →
New request → nombre → completar URL → Send**. La colección se crea en una carpeta
nueva dentro de la ubicación elegida; la petición se guarda en su raíz como `.yml`
y se abre automáticamente. En **New request** solo el nombre es obligatorio;
el método usa GET por defecto, la URL puede completarse después y el archivo se
deriva del nombre. La carpeta destino es la seleccionada en el árbol, sin un
selector adicional. Al crear una colección o carpeta, el nombre de directorio
se propone a partir del nombre visible y puede editarse. No se sobrescriben
destinos existentes; los ejemplos incluidos siguen siendo de solo lectura. Las nuevas peticiones también
admiten URLs con variables como `{{baseUrl}}/users`.

También puedes usar **Open collection → seleccionar YAML → Send → inspeccionar
Body/Headers**. El selector nativo abre la carpeta que contiene `opencollection.yml`.
El árbol se indexa en Rust; las peticiones se leen al seleccionarlas.

El selector **Workspace** del navbar permite crear y cambiar espacios locales.
El espacio predeterminado es **My Workspace**; el botón de inicio vuelve a la
vista inicial y exige guardar o descartar cambios pendientes.
El menú del selector reúne la lista de workspaces, **Create workspace** y
**Manage workspaces**. El activo se identifica con un check y el acento esmeralda.
**Manage workspaces** abre una vista en el área principal con acciones de
renombrar y eliminar para cada espacio, incluido uno inactivo. Su módulo se carga
al abrirlo y se reutiliza después; un fallo conserva el editor y su borrador.
**Back to workspace**
regresa al editor conservando las pestañas y los cambios pendientes.
Renombrar conserva la selección, los archivos y la respuesta del editor.
Eliminar pide confirmación y quita solo el registro y sus referencias. Si era
el workspace activo, vuelve al predeterminado (conservando su nombre si lo
renombraste); eliminar uno inactivo conserva la selección actual. Si ya
no existe, se crea un **My Workspace** vacío. Eliminar el último workspace
siempre deja ese espacio vacío disponible.

Cada workspace conserva varias colecciones, con árboles independientes y una
colección activa para editar y ejecutar. **New collection** y **Open collection**
añaden la carpeta al workspace actual; abrir la misma carpeta no duplica su
referencia. La opción **Remove** del menú de una colección solo quita su referencia
del workspace, sin borrar archivos. Una carpeta movida o inaccesible conserva su referencia y
muestra un aviso para poder quitarla y abrir su nueva ubicación.

Los nombres, referencias de carpetas, acceso de solo lectura y selección activa
se restauran al iniciar desde `workspaces.json` en los datos locales de Tauri.
El formato actual es versión 2 y carga los registros de versión 1; esta
migración conserva las referencias y actualiza la etiqueta original del workspace.
Este archivo versionado se escribe mediante un temporal con permisos 0600 en
Unix, fuera de las colecciones. No guarda peticiones, respuestas, secretos ni
borradores. Un archivo corrupto o cambiado externamente se conserva y bloquea
las escrituras de referencias. Se presupone una instancia de la app escribiendo
este registro. No hay variables de workspace ni sincronización entre equipos.

Para probar el ejemplo incluido, inicia `node examples/basic-http/server.mjs`
desde la raíz y pulsa **Try example**. Se seleccionan `users/list.yml` y el
environment `local`. La misma petición puede ejecutarse con la
[CLI](../../bins/cli/README.md).

El botón **+** del workbench crea un request **Untitled** en memoria. Su primer
**Save** pide nombre, colección y carpeta; las peticiones ya guardadas usan su
archivo actual. El nombre también se edita con doble clic en el breadcrumb.
Los tabs tienen un ancho máximo de 240 px y truncan el nombre; al desbordarse,
los chevrons permiten recorrer los tabs ocultos y **+** sigue visible. Al abrir
o activar un request, su pestaña se muestra automáticamente.

Un tab nuevo sin editar se cierra directamente, sin indicador de cambios ni
confirmación. Un request con cambios pendientes muestra **Save / Close without
saving / Cancel** al cerrar; si aún no tiene archivo, Save pasa al selector de
destino. Un fallo o cancelación del guardado conserva la pestaña y el borrador.
Al reiniciar se recuperan las pestañas de la última sesión del workspace activo,
su orden, la pestaña seleccionada, el environment elegido y los borradores,
incluidos los **Untitled**. Las revisiones se obtienen de nuevo al abrir cada
archivo. Si cambió o desapareció, la configuración recuperada queda como un
request sin guardar; Save permite crear una copia completa en otra ubicación
sin sobrescribir el original ni perder campos no soportados.

La copia automática se escribe tras 400 ms sin cambios en `recovery.json`,
fuera de las colecciones, mediante un temporal sincronizado y permisos 0600 en
Unix. Guarda la configuración del request, incluidos bodies y valores literales
escritos en él; usa placeholders para credenciales. No incluye respuestas,
secretos de ejecución, baseUrl override ni borradores de environments. El límite
del respaldo es 128 pestañas y 16 MiB. Un archivo corrupto, incompatible,
redirigido por un enlace o cambiado externamente se conserva y bloquea su
sobrescritura; la app muestra un aviso. Se presupone una instancia escribiendo
el respaldo. Una salida forzada recupera la última copia completada.
Si una escritura tarda, se conserva solo la que está en curso y el respaldo
pendiente más reciente. El cierre espera ese respaldo final y un fallo mantiene
la ventana abierta cuando quedan borradores.

Las pestañas abiertas conservan sus respuestas cargadas. Reenviar elimina el
cuerpo anterior de la pestaña; cerrarla libera también la respuesta de Rust si
su ID sigue siendo el actual. Cerrar una respuesta antigua no elimina la nueva.

El menú de opciones de cada request en el árbol ofrece **Delete** tanto en la
raíz de una colección como dentro de una carpeta. Pide confirmación antes de
borrar permanentemente el archivo y cerrar su pestaña; también avisa si perderá
cambios pendientes. Cancelar o un fallo conserva el request y sus cambios.
El borrado respeta el acceso de solo lectura y bloquea archivos cambiados desde
la revisión abierta. Las carpetas y las demás peticiones se conservan.

Los tabs Params, Auth, Headers, Body y Variables editan la petición. Se conservan
filas duplicadas y flags de habilitación. Params, Headers y Variables de request
siempre ofrecen una fila vacía al final y añaden la siguiente al editarla; no hay
botón de agregar y la fila virtual no se guarda ni marca cambios pendientes.
Los bodies soportados son JSON, texto y XML, y las auth Basic, Bearer, API Key
e Inherit. Body carga CodeMirror 6 al abrir un tipo soportado: números de línea,
resaltado JSON/XML, indentación de dos espacios, búsqueda/reemplazo y undo/redo.
JSON cierra automáticamente llaves, corchetes y comillas, deja el cursor entre
ambos caracteres y salta un cierre ya insertado. Backspace entre un par vacío
borra ambos caracteres. El editor conserva el borde neutro y no resalta la línea activa.
Ctrl/Cmd+F abre búsqueda; Ctrl/Cmd+[ y ] ajustan indentación; Tab conserva la
navegación entre controles salvo cuando confirma una sugerencia. Solo hay una instancia del editor montada.
Cambiar de request o salir de Body conserva el borrador y libera el historial
de undo de esa vista. **Discard** también reinicia el historial.
**Format JSON** es explícito y reversible: conserva números grandes, claves
duplicadas y escapes; si el JSON es inválido o excede los límites del formateador,
deja el texto intacto. Abrir, guardar o cambiar de tipo no formatea el contenido,
incluidos Unicode y finales de línea CRLF/CR/LF. Los saltos de línea al pegar usan
la convención del documento. Un fallo de carga deja un textarea editable y
**Retry editor**, conservando el borrador.
Escribir `{{` abre sugerencias de variables y filtra por prefijo; flechas,
Enter o Tab seleccionan una opción y completan el cierre sin duplicar llaves.
Pegar un prefijo también abre el menú. Si no hay variables o coincidencias,
el menú ofrece agregarlas en **Variables**; si falla el contexto heredado, lo
indica en lugar de ocultar las sugerencias.
El menú indica el scope y el archivo de origen, con la misma precedencia del
core: colección → carpetas → environment → request → override de runtime.
Los placeholders definidos se resaltan en morado; los ausentes, en rojo.
Hover o Alt+Enter con el cursor dentro de un placeholder abre su valor editable.
**Apply** modifica la variable del request o crea un override si es heredada;
**Save** persiste el borrador y **Discard** lo descarta. El override de `baseUrl`
sigue siendo transitorio. Los secretos muestran un campo password vacío para
reemplazar su valor solo en memoria, sin mostrarlo ni guardarlo en YAML/historial.
El selector **Environment** reúne la selección, **Create environment** y
**Manage environments**, con el mismo menú que Workspace. La creación pide solo
el nombre, selecciona el environment nuevo y cierra el formulario. La gestión
abre una página propia con lista lateral y búsqueda, limitada a los environments
de la colección activa, y conserva el request al regresar. La tabla permite
editar Name, Value y Description, y agrega una
fila vacía al escribir, como Variables de request, y permite habilitar, editar,
eliminar y declarar secretos. Save/Reset protege los cambios al regresar o
seleccionar otro environment. La selección aparece marcada en la lista; Save y
Reset guardan o descartan el borrador.
`baseUrl override` es transitorio y tiene prioridad sobre el YAML.

**Send** o Ctrl/Cmd+Enter ejecuta el borrador mediante el core
Rust; **Cancel** cancela la operación. El panel Response aparece al recibir una
respuesta y presenta status, duración, tamaño y vistas Body/Headers. Su altura
se ajusta arrastrando el separador, con límites para conservar espacio para el
editor; la preferencia se guarda localmente.
El módulo de Response y sus herramientas JSON se cargan con la primera respuesta;
las siguientes reutilizan el módulo. Si falla su carga, quedan disponibles el
cuerpo original, los encabezados, las métricas HTTP y los siguientes fragmentos.

JSON usa **Pretty** por defecto cuando el body completo es válido, con colores
por tipo y objetos/arrays plegables. También ofrece **Text**, **Raw**, ajuste de
líneas y copia; las respuestas binarias se muestran como **Hex**. El formateado
tiene límites de tamaño/profundidad y no cambia los valores numéricos originales.
Copiar/guardar/recargar confirma con un icono de check temporal.
El body se conserva en Rust y se solicita por fragmentos de 64 KiB, con carga
adicional manual y máximo de 16 MiB. El backend retiene solo la última respuesta;
la copia completa requiere cargar todo el body.

Los comandos Tauri usan una sesión con una raíz elegida por el usuario. JS no
accede directamente al filesystem ni ejecuta HTTP. HTML se presenta como texto.
**Save** o Ctrl/Cmd+S guarda solo los campos editados. **Discard** recarga desde
disco; en un Untitled restablece el request vacío. Cambiar de pestaña conserva
su borrador. Antes de cambiar de workspace o abandonar colecciones con cambios
pendientes, el editor exige guardar o descartar esos cambios. Al cerrar la
ventana guarda y espera la copia de recuperación más reciente; un fallo del
respaldo bloquea el cierre si hay borradores editados. Los cambios pendientes
en environments aún requieren Save o Discard antes de cerrar. Los tabs nuevos
sin editar no bloquean estas acciones.
Los ejemplos incluidos son de solo lectura; abre `examples/basic-http` con el
selector de carpetas para editar los archivos originales.

El backend conserva bytes y CST, compara la revisión, valida el resultado,
sincroniza un temporal del mismo directorio y reemplaza el archivo. Guardar sin
cambios no altera bytes ni mtime. Se preservan permisos, comentarios, campos
desconocidos y formato de regiones no editadas; nodos nuevos pueden usar YAML
flow. Un cambio externo o borrado bloquea el guardado y exige recargar. Si se
detecta un conflicto después de escribir el temporal, se conserva su ruta para
recuperación. Existe una ventana entre la última comprobación y el reemplazo
frente a escritores externos que no cooperan; no se promete compare-and-swap.

Cada envío relee defaults y el environment. Un borrador con una revisión de
petición distinta a la del disco se bloquea. No hay guardado de varios archivos
como una transacción ni soporte de colecciones bundled.

El watcher nativo `notify` vigila las colecciones del workspace activo y sus
directorios padre para detectar altas, cambios, renombres, borrados y reemplazos
de la raíz desde un editor o Git. No sigue symlinks e ignora las rutas ocultas,
`.git`, `node_modules`, `scripts`, `fixtures` y `auth`, como el índice del árbol.
Las notificaciones se agrupan tras 300 ms sin eventos; la actualización espera
si hay HTTP, guardados, edición de nombre o un diálogo abierto. Al cambiar de
workspace se retiran las suscripciones anteriores.

Actualizar el árbol conserva la colección activa y sus revisiones. Los tabs
sin cambios se recargan, y los editados conservan su borrador, incluidos los
inactivos. Un cambio externo, incluso solo de comentarios, muestra **Reload
from disk / Save copy** y bloquea Save y Send hasta resolverlo. Reload descarta
explícitamente el borrador seleccionado; Save copy conserva la configuración
completa en un request sin guardar y abre el selector de destino. Un archivo
borrado o renombrado se conserva automáticamente como copia sin guardar.
Un YAML ilegible o una colección inaccesible mantiene el contenido abierto y
muestra un aviso; puede recuperarse al corregir el archivo.

El botón **Refresh collections** y volver a enfocar la ventana vuelven a leer
el workspace. Sirven también cuando el filesystem no emite notificaciones
fiables o el watcher informa un error. La lectura y las notificaciones nativas
se verifican en macOS y Linux ARM64 bajo Docker; Windows sigue pendiente.

## Política de secretos e historial del MVP 0

Los valores de **Secrets · memory only** se suministran únicamente para Send.
Se eliminan del estado al cambiar de environment o colección y no se escriben
en YAML ni en historial. La CLI obtiene secretos de variables de proceso
nombradas explícitamente con `--secret-env`; no importa todo el environment.
Save escribe las plantillas y los valores de configuración editados: usa
`{{token}}`/`{{password}}` para mantener credenciales fuera de los archivos.
El body configurado sí se guarda en el YAML al pulsar Save.

**History** persiste solo fecha UTC, ruta relativa de la petición, método,
status/categoría del resultado, duración y número de bytes recibidos. No incluye
URL, query resuelta, valores de environment, credenciales, headers, mensajes de
error ni bodies enviados/recibidos. El método se limita a nombres estándar o
CUSTOM; la ruta identifica el archivo, sin almacenar la raíz de la colección.

Se retienen las últimas 200 ejecuciones, en `history.json` bajo el directorio
de datos de Tauri (`dev.nimblepost.desktop`), con reemplazo atómico y permisos
0600 en Unix. **Clear history** vacía los registros. Un archivo incompatible o
corrupto al abrirse se conserva y se informa; no se sobrescribe automáticamente.
Un fallo al persistir historial no convierte una respuesta HTTP exitosa en error.
Se presupone una instancia de la app escribiendo ese historial.

Persistir bodies, headers o credenciales en el historial requerirá una decisión
separada con consentimiento explícito, redacción, retención y borrado definidos;
no existe esa opción en MVP 0. No hay almacenamiento persistente de secretos ni
integración con keychain todavía.

```sh
npm run check
npm run build
cargo test --workspace --locked
```

## Renderizado de árboles y respuestas

El árbol monta las filas del viewport y seis filas adicionales por extremo.
Al desplazarse reutiliza esas filas y sus componentes; las rutas, permisos,
etiquetas y atributos de accesibilidad se actualizan con su contenido. El foco
de menús y diálogos conserva la identidad lógica del request, incluso si su
botón se reutilizó mientras estaba fuera de pantalla.
Las colecciones y carpetas cerradas omiten sus descendientes. Seleccionar una
pestaña revela su request y abre sus carpetas; el estado de expansión se conserva
al hacer scroll y refrescar archivos. El árbol usa una sola parada de Tab:
flechas para recorrer/expandir, Enter para abrir y Shift+F10 para sus acciones.
Los nombres y métodos se leen solo para esas filas y se reutilizan al volver.
La caché y los intentos de lectura se limitan a 512 rutas por colección, o las
filas visibles si su número supera ese límite. Las rutas ocultas desalojadas
se vuelven a consultar al aparecer; también se desalojan los intentos fallidos.
Mientras llegan las etiquetas se muestra el nombre del archivo y `HTTP`.
Las notificaciones externas identifican colecciones: se invalidan sus etiquetas
y se releen las visibles; las ocultas esperan hasta aparecer. Las demás
colecciones conservan su caché. Un YAML inválido o una lectura fallida mantiene
el nombre del archivo sin reintentos continuos; Refresh collections permite
reintentarlo. Las etiquetas del request activo reflejan su borrador.

Las respuestas de más de 500 líneas montan las líneas visibles y seis líneas
adicionales por extremo. El índice guarda posiciones dentro del cuerpo original;
las alturas de las líneas ajustadas se miden al aparecer. El cuerpo completo sigue
disponible mediante Copy response body. Pretty conserva el formato y los valores
en respuestas grandes, pero omite resaltado y plegado cuando supera ese umbral.
Los cuerpos pequeños conservan su presentación anterior. El visor de respaldo
ante un fallo de importación usa el mismo límite. La selección y búsqueda nativas
del navegador solo abarcan el texto montado; la copia completa usa el botón.
Esta optimización limita líneas, no caracteres de una única línea muy larga.

## Rendimiento y distribución nativa

El [RFC-0010](../../docs/rfc/0010-presupuestos-rendimiento.md) conserva el protocolo
de rendimiento como referencia. Las herramientas y reportes de medición se
retiraron del repositorio.

`tauri.windows.conf.json` configura NSIS y `tauri.linux.conf.json` configura
Debian. CI tiene runners Windows 2025/Ubuntu 24.04 x64 para compilar, instalar y
abrir paquetes y ejecutar las pruebas Rust. Los resultados de estos runners x64
siguen pendientes hasta ejecutar los jobs del commit correspondiente. Los artefactos de CI no cambian
el canal de releases públicas, actualmente macOS ALPHA.
