# RFC-0003 — Contrato inicial de almacenamiento OpenCollection

Estado: lectura, HTTP, edición y guardado de requests/environments implementados en MVP 0.
Fecha: 2026-09-30.

El alcance ejecutable y sus límites actuales están documentados en
[crates/core](../../crates/core/README.md). La tabla siguiente sigue definiendo
el contrato objetivo; bundled, edición de defaults y funciones fuera del subconjunto siguen pendientes.

## Versión y referencia

El formato aceptado inicialmente es **OpenCollection `1.0.0`**, escrito como
string en el archivo raíz `opencollection.yml`. No se acepta automáticamente
cualquier `1.x` ni se migra una colección al abrirla.

Fijamos el esquema de `@opencollection/schema` **`0.14.0`**, revisión
`535a5c5803da7a3fe1d575fff23350b4a7c43c6a` del repositorio oficial. La versión del
paquete no es la versión del formato. La copia íntegra y su SHA-256 están en
[schemas/opencollection/1.0.0](../../schemas/opencollection/1.0.0/README.md).

Fuentes primarias:

- [Esquema fijado](https://github.com/opencollection-dev/opencollection/blob/535a5c5803da7a3fe1d575fff23350b4a7c43c6a/packages/oc-schema/src/opencollection.schema.json).
- [Ejemplos publicados](https://github.com/opencollection-dev/opencollection/tree/535a5c5803da7a3fe1d575fff23350b4a7c43c6a/packages/oc-spec-site/src/components/sections).
- [Fixtures reales de Bruno](https://github.com/usebruno/bruno/tree/bc90e46b288446ebe0c8d67b28470d2efc0a8c48/packages/bruno-cli/tests/runner/fixtures/opencollection/collection).

El esquema es la referencia estructural. Las reglas de ejecución y guardado que
siguen son decisiones de NimblePost; no se atribuyen a OpenCollection ni implican
que reproducimos toda la semántica de Bruno.

## Lectura, validez y ejecución son capacidades distintas

| Estado | Comportamiento previsto |
|---|---|
| YAML válido + esquema válido + subconjunto soportado | Abrir, editar y ejecutar cuando el backend esté implementado |
| YAML válido + funciones válidas aún no soportadas | Conservar y mostrar; bloquear ejecución si la función activa afecta la petición |
| YAML válido + campos desconocidos o incompatibles | Conservar el documento, informar ubicación; bloquear ejecución afectada, sin conversión automática |
| YAML ambiguo o inválido | Mostrar diagnóstico y texto; bloquear edición estructurada y ejecución |
| Versión raíz ausente o diferente de `1.0.0` | Abrir como texto conservado; bloquear interpretación, ejecución y edición estructurada |

Validar contra el esquema no basta: su raíz no exige `opencollection`, y varias
definiciones permiten objetos incompletos. NimblePost exige nombre de colección,
tipo de petición, método y URL antes de ejecutar. Un archivo Environment o
HttpRequest separado se valida contra su definición, no contra la raíz permisiva.

Los campos desconocidos nunca se eliminan para conseguir una validación positiva.
Se podrá editar un campo soportado en un documento incompatible solo si se
conservan los demás nodos y no se introducen nuevos diagnósticos. Conservar
datos ajenos no los convierte en válidos o ejecutables.

## Layout

Se admitirán dos formas de almacenamiento:

```text
Bundled: un YAML con opencollection, bundled: true e items anidados.

Filesystem:
api/
  opencollection.yml
  environments/
    local.yml
  users/
    folder.yml
    list.yml
    create.yml
```

- `opencollection.yml` identifica la colección y contiene defaults y extensiones.
- Un request separado contiene `info`, `http`, `runtime`, `settings`, etc.; no
  necesita repetir `opencollection`. Su versión proviene de la raíz.
- `folder.yml` contiene metadatos y defaults de carpeta. En filesystem, el nombre
  reservado identifica la carpeta aunque falte `info.type`; NimblePost escribe
  `info.type: folder` en carpetas nuevas. Los folders bundled requieren ese tipo.
- `environments/*.yml` son documentos `Environment`, con `name` y `variables`.
- En bundled, los environments están en `config.environments` y las carpetas en
  `items`. No se mezclan con archivos externos en la primera implementación.
- En filesystem se descubren requests/carpetas por disco. Un `items` o
  `config.environments` no vacío en la raíz se conserva, pero el layout mixto no
  se interpreta ni ejecuta inicialmente. `bundled: true` no activa discovery.
- `bundled` ausente en `opencollection.yml` significa filesystem para NimblePost.
  No se cargan archivos auxiliares como requests; `.git` y `node_modules` se
  excluyen. Dentro de `environments`, únicamente se buscan environments.
- Orden visual: `info.seq`, después ruta relativa como desempate. La identidad
  sigue siendo ruta + revisión; `seq` y el nombre visible no son IDs.

No hay manifiesto propietario ni IDs obligatorios. Las extensiones nuevas de la
colección se ubican en `extensions.nimblepost`. El esquema sí declara
`extensions` en la raíz; no declara extensiones arbitrarias en todos los objetos.
No se usa `x-project` como formato nuevo.

## Campos del primer motor HTTP

La tabla define el alcance de implementación; todavía no afirma soporte runtime.

| Ubicación | Interpretación inicial |
|---|---|
| `info.name`, `info.type`, `info.seq` | Nombre, discriminante y orden; descripción/tags se conservan |
| `http.method`, `http.url` | Método HTTP y URL absoluta tras interpolación, solo HTTP/HTTPS |
| `http.headers[]` | `name`, `value`, `disabled`; arrays para conservar orden y duplicados |
| `http.params[]` | Query: `name`, `value`, `type: query`, `disabled` |
| `http.body` | Ausente o un objeto `type: json/text/xml` con `data` string |
| `http.auth` | `basic`, `bearer`, `apikey` o string `inherit` |
| `request.headers/auth/variables` | Defaults de colección y carpeta |
| `runtime.variables[]` | Variables de la petición; string directo o valor tipado string |
| `config.environments[]` / archivo Environment | Environment seleccionado por nombre único |
| `settings` / `request.settings.http` | Timeout, redirects, encodeUrl, omitHeaders y valores `inherit` |
| `docs`, `examples`, `extensions` | Preservación; no se ejecutan |

Se exige `info.type: http` y `http.method/url` no vacíos. El método debe ser un
token HTTP válido. Los valores de headers no pueden contener CR/LF. El body JSON
se valida después de interpolar y no se reformatea durante el guardado.

Los headers de un scope más específico reemplazan el grupo del mismo nombre
(comparación sin distinguir mayúsculas). Dentro de ese grupo se conservan los
duplicados y su orden; filas `disabled: true` no se envían. Un grupo local
completamente deshabilitado suprime el grupo heredado. Query params se añaden a
los presentes en la URL, mantienen duplicados y se codifican por componente.

Auth explícita se toma de `http.auth`, después del `request.auth` de la carpeta
más cercana y finalmente de la colección. Ausencia o `inherit` continúa la
búsqueda; sin configuración resuelta no hay auth. La primera versión no ofrece
un override propietario de “No Auth” para anular un ancestor. Basic exige
username/password, Bearer token y API Key key/value/placement. Un header o query
manual que choque con el campo generado por auth produce error, no un override
silencioso. Credenciales se resuelven en memoria.

Settings se heredan campo por campo: petición → carpeta más cercana → colección
→ defaults NimblePost. Defaults: timeout 30,000 ms, encodeUrl true,
followRedirects true, maxRedirects 5, forwardAuthorizationHeader false,
omitHeaders vacío. Timeout debe ser un entero positivo y maxRedirects entero
entre 0 y 20. Nunca se reenvía auth a otro origen en el primer motor; una
configuración activa que lo exija se declara no soportada.

Se conservan, pero se bloquea la ejecución afectada por: GraphQL/gRPC/WebSocket,
path params activos, multipart/file/form-urlencoded, variantes de body, variables
tipadas no string y variantes, OAuth y otras auth, scripts/assertions/actions
activos, proxies/certificados explícitos, environment `extends`, `dotEnvFilePath`
y externalSecrets. Un array vacío no activa una función. Un elemento
`disabled: true` se omite donde el esquema reconoce ese flag.

## Variables: reglas propias de NimblePost

Precedencia inicial, de mayor a menor:

```text
overrides transitorios de la ejecución
runtime.variables de la petición
variables del environment seleccionado
request.variables de la carpeta más cercana (hacia sus ancestors)
request.variables de la colección
```

Global y workspace se posponen. La opción llamada `runtime.variables` en YAML
es el scope de petición: no se confunde con overrides transitorios en memoria.
Cada resolución devuelve valor y procedencia (archivo, scope y nombre), para
que la UI pueda explicar el resultado. Secretos devuelven procedencia, sin
exponer el valor en logs, errores o metadatos IPC.

- Sintaxis inicial: `{{name}}`; se permite whitespace alrededor del nombre.
  Nombres case-sensitive con letras ASCII, números, `_`, `-` o `.`, no vacíos.
- Solo string directo o `{type: string, data: "..."}`. `disabled: true` elimina
  esa entrada del scope; permite buscar el mismo nombre en scopes inferiores.
- Duplicados activos dentro del mismo scope o environments con el mismo nombre
  son errores. No se aplica “el último gana” por orden accidental.
- Sustitución de una pasada, en URL, headers, params, body y campos de auth.
  Los valores de variables no se interpolan recursivamente; un valor que
  contenga otro placeholder produce error de configuración inicialmente.
- Variable ausente, placeholder mal formado o environment inexistente: error
  antes de red. No se sustituyen por string vacío ni se envía el placeholder.
- No hay eval, expresiones JS, funciones dinámicas ni lectura implícita de todas
  las variables de entorno del proceso.
- Un environment puede declarar `{name: token, secret: true, type: string}` sin
  `value`. El primer proveedor será un mapa explícito de secretos suministrado
  en memoria por el adaptador. Declaraciones activas requieren nombre y un valor
  suministrado cuando una plantilla los utiliza; no se leen proveedores externos
  automáticamente. Una declaración no usada no impide ejecutar otra petición.
- Un secret participa en el scope del environment, no en una precedencia global
  distinta. Choques con una variable activa del mismo nombre son errores.
- Nunca se persiste la petición resuelta ni se reemplaza `{{token}}` con el
  secreto al guardar. Previews de campos sensibles se redactan.

## Guardado sin pérdida de campos desconocidos

El backend conservará **texto original + árbol de sintaxis con posiciones + vista
de dominio**. Deserializar a structs y serializar de nuevo no satisface el
contrato: perdería nodos ajenos, comentarios y formatos.

1. Abrir o guardar sin cambios no modifica bytes, mtime, comentarios ni newline.
2. Editar modifica exclusivamente el nodo solicitado. Campos desconocidos,
   extensiones de otros clientes y bloques no soportados se conservan completos.
3. Comments, orden de keys, quoting y newline de regiones no editadas permanecen
   idénticos. El valor editado puede necesitar nuevas comillas o representación.
4. La API de edición usa una ruta de campo y revisión del documento. No acepta
   como reemplazo de archivo la proyección reducida enviada por la UI.
5. Antes de escribir se comparan los bytes actuales con la revisión abierta.
   Un cambio externo o borrado produce conflicto; no se sobrescribe ni recrea.
6. Se escribe a un temporal del mismo directorio, se valida, se sincroniza y se
   reemplaza con el mecanismo atómico de la plataforma. Se preservan permisos.
   No se afirma atomicidad de una edición de varios archivos.
7. El guardado no debe introducir diagnósticos estructurales nuevos. Archivos
   incompatibles no se “reparan” silenciosamente ni se fuerzan a pasar el schema.
8. Se serializan plantillas y valores de configuración, nunca los overrides,
   secretos suministrados ni credenciales resueltas.

Un bloqueo del backend serializa sus propios escritores. Se vuelve a comprobar
la revisión inmediatamente antes del reemplazo. Ese bloqueo no obliga a un
editor externo a cooperar: queda una ventana entre comprobación y reemplazo.
La implementación deberá documentar ese límite y conservar el temporal cuando
detecte un conflicto. No se promete compare-and-swap portable ni protección
absoluta frente a escritores externos que actúen dentro de esa ventana.

YAML inicial: UTF-8, un documento, YAML 1.2 con keys string únicas. Anchors,
aliases, merge keys, tags personalizados y directivas 1.1 quedan fuera de la
edición estructurada inicial: se preserva el texto y se informa. Parseo y
lectura tendrán un límite inicial de 8 MiB por archivo; un archivo mayor puede
abrirse como texto, pero no se parsea ni ejecuta en este primer backend.

## Evidencia y límites de compatibilidad

Los [fixtures y su manifest](../../tests/fixtures/opencollection/manifest.json)
conservan ejemplos publicados y una colección real de los tests de Bruno.
Cada archivo tiene origen, revisión, SHA-256 y resultado esperado.

Se detectaron dos incompatibilidades concretas en esa colección real:

- `request.headers[0].enabled`: el esquema solo reconoce `disabled`.
- `http.body.mode/json`: el esquema espera `type/data`.

No se normalizan en los originales. Los ejemplos propios de
[basic-http](../../examples/basic-http/opencollection.yml) usan campos válidos
del esquema fijado. No se anuncia compatibilidad total con Bruno por aceptar
su nombre de formato. Tampoco se copian ejemplos de documentación sin validarlos:
el ejemplo publicado de Environment incluye `transient`, ausente del schema.

## Verificación y siguiente implementación

```sh
npm ci
npm test
```

Las pruebas actuales verifican integridad del schema y fixtures, validez según
la definición apropiada, incompatibilidades conocidas y forma básica de los
ejemplos propios. Son tooling de desarrollo; no son parser ni motor de producto.

El núcleo Rust ya prueba el corpus real, precedencia/procedencia, variables
ausentes/duplicadas, secretos suministrados en memoria, lectura sin modificar
archivos, rechazo de YAML ambiguo, límites y ejecución HTTP local con timeout
y cancelación. Las pruebas de storage ahora verifican no-op byte a byte y mtime,
edición de scalars/arrays/body/auth, conservación de unknowns/comments/CRLF,
edición de fixtures reales, conflicto por cambios externos/borrado y fallo con
un archivo de solo lectura sin perder el original.

MVP 0 usa `yaml-edit` 0.3.2 para el CST y sus posiciones. Los valores existentes
se sustituyen en su span; los nuevos se insertan como nodos YAML flow. Se conserva
el separador de los bloques al editar y se vuelve a parsear con yaml-rust2 para
comprobar equivalencia con las únicas rutas solicitadas. El guardado se bloquea
si el editor no conserva el documento o introduce diagnósticos nuevos.
La API Tauri acepta revisión + cambios de campo, nunca un reemplazo de archivo.
No hay guardado estructurado de colección/carpeta ni transacciones multiarchivo.

El historial tiene un contrato independiente de la colección: la
[política del MVP 0](../../apps/desktop/README.md#política-de-secretos-e-historial-del-mvp-0)
excluye credenciales, URLs, headers y bodies y limita la retención a 200 registros.
