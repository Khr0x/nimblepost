# Esquema OpenCollection fijado

`opencollection.schema.json` es una copia íntegra, sin cambios, del esquema
publicado por OpenCollection. No es un esquema propietario de NimblePost.

- Formato: OpenCollection `1.0.0`, JSON Schema draft-07.
- Paquete fuente: `@opencollection/schema` `0.14.0`.
- Revisión: `535a5c5803da7a3fe1d575fff23350b4a7c43c6a`.
- [Origen](https://github.com/opencollection-dev/opencollection/blob/535a5c5803da7a3fe1d575fff23350b4a7c43c6a/packages/oc-schema/src/opencollection.schema.json).
- Integridad SHA-256: registrada en el [manifest](../../../tests/fixtures/opencollection/manifest.json)
  y comprobada por `npm test`.
- Licencia: MIT según el [package.json original](https://github.com/opencollection-dev/opencollection/blob/535a5c5803da7a3fe1d575fff23350b4a7c43c6a/packages/oc-schema/package.json).
  Autor declarado: Bruno Software Inc. El snapshot no incluye un archivo LICENSE
  ni un aviso de copyright en el esquema. Véase [la atribución](../../../THIRD_PARTY_NOTICES.md).

Cambiar el esquema requiere revisar el RFC-0003, actualizar revisión y hash, y
comprobar de nuevo todos los fixtures. El mismo string `1.0.0` puede aparecer en
revisiones distintas del esquema: por eso también fijamos paquete y commit.

No se obtiene un esquema remoto durante la validación. `npm ci` requiere instalar
las herramientas de desarrollo; después `npm test` funciona sin red.
