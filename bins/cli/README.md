# CLI mínima

El binario `nimblepost` lee y ejecuta una petición con `nimblepost-core`.
Desde la raíz del repositorio, inicia la API de ejemplo en otra terminal:

```sh
node examples/basic-http/server.mjs
```

Ejecuta el mismo YAML que abre Desktop:

```sh
cargo run -p nimblepost-cli -- run users/list.yml --collection examples/basic-http --env local
```

`--var NAME=VALUE` permite overrides transitorios, repetibles con nombres
distintos. Para un archivo suelto, omite `--collection` y suministra sus variables:

```sh
cargo run -p nimblepost-cli -- run examples/basic-http/users/list.yml --var baseUrl=http://127.0.0.1:3000 --var pageSize=5
```

Sin colección, no se cargan defaults, environments ni variables de carpeta.
La salida JSON contiene `status`, `headers`, `body`, `bodyEncoding` y `bodyBytes`.
Los headers conservan duplicados y su valor original en `valueBase64`; un body
UTF-8 se entrega como texto, y el resto como base64. Los errores van a stderr.

Ctrl+C cancela la petición. Códigos de salida: 0 para una respuesta HTTP
(incluidos 4xx/5xx), 1 para fallo de carga/preparación/transporte, 2 para argumentos
inválidos y 130 para cancelación. Timeout y límites son los del
[núcleo](../../crates/core/README.md).

Para una declaración `{name: token, secret: true, type: string}` del environment,
`--secret-env token=NIMBLEPOST_TOKEN` suministra el valor de esa variable de
proceso en memoria. Debe existir; no se carga el resto del environment ni se
escribe el secreto en YAML. No se pasa el valor por argumentos de la CLI.

```sh
cargo run -p nimblepost-cli -- run users/create.yml --collection examples/basic-http --env local --secret-env token=NIMBLEPOST_TOKEN
```

Esta CLI ejecuta una petición por llamada. El historial y el editor/guardado se
exponen inicialmente desde Desktop; el motor y storage siguen en el core
compartido. La ejecución de colecciones completas queda pendiente.
