# Desktop: Tauri 2 + Svelte 5

**Estado al 2026-10-01:** MVP 0 HTTP funcional en desarrollo ALPHA (`0.1.0`),
con workspaces locales, múltiples colecciones y edición en tabs.

El mapa de carpetas y sus responsabilidades está en
[RFC-0002 — Organización del repositorio](../../docs/rfc/0002-arquitectura-inicial.md#organización-del-repositorio--2026-10-01).
La UI se organiza en `src/lib/features`, el puente IPC en `src/lib/desktop`,
los componentes compartidos en `src/lib/components` y los estilos en `src/styles`.
Las pruebas locales están en `tests/unit` y `tests/ui`.

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
renombrar y eliminar para cada espacio, incluido uno inactivo. **Back to workspace**
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
Los borradores no se restauran después de reiniciar la aplicación.

Los tabs Params, Auth, Headers, Body y Variables editan la petición. Se conservan
filas duplicadas y flags de habilitación. Params, Headers y Variables de request
siempre ofrecen una fila vacía al final y añaden la siguiente al editarla; no hay
botón de agregar y la fila virtual no se guarda ni marca cambios pendientes.
Los bodies soportados son JSON, texto y XML, y las auth Basic, Bearer, API Key
e Inherit. **Edit environments** permite
crear environments, modificar variables públicas y declarar secretos string.
`baseUrl override` es transitorio y tiene prioridad sobre el YAML.

**Send** o Ctrl/Cmd+Enter ejecuta el borrador mediante el core
Rust; **Cancel** cancela la operación. El panel Response aparece al recibir una
respuesta y presenta status, duración, tamaño y vistas Body/Headers. Su altura
se ajusta arrastrando el separador, con límites para conservar espacio para el
editor; la preferencia se guarda localmente.

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
pendientes, o cerrar la ventana, el editor exige guardar o descartar esos cambios.
Los tabs nuevos sin editar no bloquean estas acciones. Forzar la salida del
proceso no conserva borradores.
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
petición distinta a la del disco se bloquea. No hay watcher, guardado de varios
archivos como una transacción ni soporte de colecciones bundled.

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

## Regresiones de UI

Con Vite iniciado, los siguientes casos ejercitan la app real con IPC y datos
simulados en memoria, sin modificar colecciones del usuario:

- `http://127.0.0.1:1420/tests/ui/close-request-tabs.html`: cierre de tabs vacíos,
  Save/Discard/Cancel, destino de guardado, fallos, edición de nombre y controles.
- `http://127.0.0.1:1420/tests/ui/request-tab-overflow.html`: 20 tabs, chevrons,
  creación, ancho máximo, truncado, ajuste al ancho y conservación de borradores.
- `http://127.0.0.1:1420/tests/ui/response-panel.html`: visibilidad, formatos JSON,
  colores, plegado, copia, carga parcial y límites del panel ajustable.
- `http://127.0.0.1:1420/tests/ui/workspace-menu.html`: selección, gestión en vista,
  rename/delete, errores, fallback y protección de cambios pendientes.

Cada caso muestra `PASS` o `FAIL` al terminar. La validación nativa de todas las
plataformas y los benchmarks de rendimiento siguen pendientes.
