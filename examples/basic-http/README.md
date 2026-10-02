# Colección HTTP de referencia

Ejemplo propio de NimblePost, válido contra el esquema OpenCollection fijado.
Incluye un servidor local de demostración sin dependencias adicionales.

`opencollection.yml` define defaults; `environments/local.yml` selecciona
`baseUrl` y declara un secreto sin valor. `users/list.yml` sobrescribe `pageSize`
de la carpeta: la preparación produce `GET /users?limit=5`.
`users/create.yml` necesita un `token` suministrado en memoria antes de ejecutar.

El ejemplo incluye comentarios, quotes, defaults, `inherit`, body JSON y una
extensión ajena para las pruebas futuras de conservación. Sus archivos se
validan con `npm test` desde la raíz del repositorio.

Inicia el servidor desde la raíz:

```sh
node examples/basic-http/server.mjs
```

En otra terminal, ejecuta la petición:

```sh
cargo run -p nimblepost-cli -- run users/list.yml --collection examples/basic-http --env local
```

Para usarla en Desktop: `npm run desktop:dev` y **Try example**. El servidor
escucha en `127.0.0.1:3000` y devuelve usuarios como JSON; Ctrl+C lo detiene.
