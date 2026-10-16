# RFC-0009 — Contrato de CLI mínima

Estado: implementado; formalizado por NP-016 el 2026-10-16 sobre `b8aa888`.
Fuente: `Args`, `Command`, `parse_variable`, `run` y `main` en
[main.rs](../../bins/cli/src/main.rs). Uso: [README CLI](../../bins/cli/README.md).
La CLI usa [el mismo core](0004-motor-http.md), sin guardar ni registrar history.

## Entrada

```text
nimblepost run <REQUEST> [-c|--collection <ROOT>] [-e|--env <NAME>]
    [--var NAME=VALUE]... [--secret-env NAME=ENV_VAR]...
```

`--env` requiere `--collection`. Con colección, REQUEST es relativo a la raíz
autorizada y carga defaults/environment. Sin ella, Document::read y standalone:
sin discovery de ancestors, environment ni secretos implícitos. Las rutas de
archivo suelto son elegidas expresamente por el caller.

Cada mapa rechaza nombres duplicados; NAME vacío o falta de `=` es argumento
inválido. `--var` es override transitorio y puede aparecer en el historial del
shell/listado de procesos: no usarlo como mecanismo de secreto. `--secret-env`
lee solo la variable de proceso nombrada; ausente/no Unicode falla antes de red.
El mapa suministrado se consume según las declaraciones de secreto del core;
no crea un scope global ni importa todo el environment del proceso.

## Salida y errores

Una respuesta HTTP imprime un único objeto JSON indentado a stdout:

```json
{
  "status": 200,
  "headers": [{"name": "x-example", "value": "ok", "valueBase64": "b2s="}],
  "body": "{}",
  "bodyEncoding": "utf8",
  "bodyBytes": 2
}
```

Headers conservan duplicados; value es texto con reemplazo para bytes inválidos,
valueBase64 conserva sus bytes. Cuerpo UTF-8 válido → body texto/bodyEncoding utf8;
otro cuerpo → body base64/bodyEncoding base64. bodyBytes cuenta bytes originales,
no caracteres/base64. Se entrega la respuesta completa al terminar; no hay NDJSON,
progreso, respuesta parcial ni reporte de varias peticiones.

| Exit code | Significado |
|---|---|
| 0 | Respuesta HTTP, incluidos 4xx/5xx; help/version también terminan correctamente |
| 1 | Carga, configuración, variables/secretos, timeout, red o límite de respuesta |
| 2 | Error de argumentos detectado por clap |
| 130 | Ctrl+C cancela la operación |

Errores de ejecución a stderr, stdout sin JSON de éxito; texto de error del core,
sin esquema JSON público de diagnósticos. Timeout 30.000 ms por defecto dentro
de execute; respuesta máxima 16 MiB. Ctrl+C interrumpe el flujo de carga/envío
y señala CancellationToken; no revierte efectos HTTP. Salidas reflejadas por el
servidor pueden contener secretos: **stdout no es un reporte redactado**.
No guardar stdout/stderr automáticamente como evidencia con datos reales.

## Compatibilidad y verificación

Sin subcomandos Git/import/runner ni escritura de colección. NP-024 definirá
runner y reportes; NP-016 no añade flags ni cambia este JSON/exit codes.
Cualquier cambio incompatible requiere decisión y actualizar consumidores/tests.
[run.rs](../../bins/cli/tests/run.rs) prueba HTTP loopback, JSON, argumentos,
variables ausentes, secretos explícitos y SIGINT (este último en Unix).
La codificación base64 se contrasta además con el código y los bytes del core;
la suite actual no acredita todos los códigos HTTP ni escenarios en cada SO.
