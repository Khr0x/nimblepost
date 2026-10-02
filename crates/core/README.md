# NimblePost core

Biblioteca Rust independiente de Desktop y CLI. Lee OpenCollection YAML 1.2,
valida contra el esquema fijado, resuelve variables y ejecuta HTTP/HTTPS con
Tokio, reqwest y rustls.

```rust,no_run
use nimblepost_core::{
    CancellationToken, ExecutionContext, execute, load_request, prepare,
};

# async fn example() -> nimblepost_core::Result<()> {
let loaded = load_request("examples/basic-http", "users/list.yml", Some("local")).await?;
let context = ExecutionContext::default();
let prepared = prepare(&loaded, &context)?;
let token = CancellationToken::new(); // otro task puede llamar a token.clone().cancel()
let response = execute(prepared, &token).await?;

assert_eq!(response.status, 200);
// response.headers conserva duplicados; response.body contiene bytes originales.
# Ok(())
# }
```

Para un archivo suelto: `Document::read(path).await` y
`LoadedRequest::standalone(document)`. Sin colección, el caller debe suministrar
las variables mediante `ExecutionContext.overrides`. Un `Document` conserva
`original()` íntegro, incluso si `validate()` detecta un campo incompatible.

Precedencia: overrides transitorios → variables de petición → environment →
carpetas desde la más cercana → colección. `variable_origins()` permite conocer
el scope y archivo sin devolver valores. Los secretos declarados se suministran
en `ExecutionContext.secrets` y solo se exigen cuando una plantilla los utiliza.
La preparación y ejecución nunca escriben archivos.

Además de método/URL se ejecutan headers, query params, body JSON/text/XML,
Basic/Bearer/API Key, defaults de carpeta/colección y settings heredados.
Configuraciones activas aún no soportadas producen error antes de enviar.

## Ejecutar el ejemplo

Con un servidor HTTP escuchando en el puerto 3000:

```sh
cargo run -p nimblepost-core --example send -- examples/basic-http users/list.yml local
```

Para usar otra URL sin cambiar YAML:

```sh
cargo run -p nimblepost-core --example send -- examples/basic-http users/list.yml local http://127.0.0.1:8080
```

El ejemplo imprime estado, headers y body como JSON legible. Su representación
del body es para texto; la biblioteca devuelve bytes y conserva respuestas
binarias. Ctrl+C cancela la ejecución. La [CLI mínima](../../bins/cli/README.md)
añade argumentos explícitos y salida binaria en base64.

## Timeout, cancelación y errores

Timeout de extremo a extremo: conexión, redirects, headers y lectura del body;
30,000 ms por defecto, sobrescribible con `settings.timeout` positivo.
Un `CancellationToken` ya cancelado impide comenzar el envío; cancelar durante
una petición descarta su future. No puede deshacer efectos ya ejecutados por el
servidor.

Los errores tipados distinguen archivo inaccesible, conflicto de revisión, YAML inválido (línea/columna),
configuración inválida (archivo/campo), variable ausente, timeout, cancelación,
conexión/transporte y límite de body. No incorporan el texto de errores de red o
parseo que podría incluir URLs o credenciales. HTTP 4xx/5xx se devuelven como
respuestas, no como fallos de transporte.

## Límites de esta entrega

- Colecciones filesystem y requests sueltos. `collection_index()` descubre el
  árbol y environments; `request_info()` expone las plantillas guardadas.
  Desktop y CLI usan esta biblioteca. Bundled y runner siguen pendientes.
- Máximo 8 MiB por YAML y profundidad 64. No aliases, anchors, tags, merge keys,
  keys duplicadas ni múltiples documentos. Se valida sin consultar schemas remotos.
- Body de respuesta en memoria, máximo 16 MiB; sin truncarlo silenciosamente.
  Desktop expone fragmentos de 64 KiB por IPC. File-backed streaming sigue pendiente.
- Proxies explícitos y del sistema desactivados en este motor. No cookies
  persistentes, OAuth, scripts, assertions ni certificados personalizados.
- Se siguen redirects dentro del mismo origen hasta `maxRedirects`; un cambio
  de origen devuelve la respuesta 3xx sin seguirla para preservar credenciales
  en headers personalizados y query params.
- `encodeUrl: false` y headers manuales de framing/transporte se rechazan.
  reqwest realiza la codificación y determina Host, Content-Length, etc.
- `Document::edited(kind, edits)` modifica rutas de campos mediante el CST de
  yaml-edit 0.3.2 y spans. Un nodo nuevo puede usar JSON flow (YAML 1.2 válido).
  Se verifica que solo cambien los valores solicitados y no haya nuevos errores
  de esquema. El parser de ejecución sigue siendo yaml-rust2.
- `save_document(original, edited)` compara bytes, conserva permisos, sincroniza
  un temporal y reemplaza un archivo. No-op no modifica mtime. Se serializan los
  escritores del backend; persiste una ventana frente a escritores externos
  entre comprobación final y rename. Los conflictos conservan el temporal si ya
  se escribió. Ver el [contrato](../../docs/rfc/0003-contrato-opencollection.md).
- `History` ofrece almacenamiento acotado de metadatos; no acepta payloads HTTP,
  URLs ni credenciales. Desktop lo utiliza según su
  [política](../../apps/desktop/README.md#política-de-secretos-e-historial-del-mvp-0).

```sh
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Las pruebas de integración usan servidores efímeros en `127.0.0.1`; no requieren
servicios externos. Desarrollo y comprobaciones realizados con Rust 1.96.1.
