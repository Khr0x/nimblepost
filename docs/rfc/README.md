# Contratos de NimblePost

Revisión NP-016: 2026-10-16, base `b8aa8889a834a3cab645e9c5d271ecb90db4e594`.
El repositorio define código y contratos; el plan SDD vive en Obsidian.
Un contrato objetivo no acredita una función implementada ni levanta los gates
de validación del cierre ALPHA (C0).

| Documento | Estado y autoridad |
|---|---|
| [RFC-0001 — Principios](0001-principios-producto.md) | Decisiones para conservar al ampliar |
| [RFC-0002 — Arquitectura](0002-arquitectura-inicial.md) | Organización implementada |
| [RFC-0003 — OpenCollection](0003-contrato-opencollection.md) | Formato/guardado; distingue runtime de objetivos |
| [RFC-0004 — Motor HTTP](0004-motor-http.md) | Contrato del motor implementado |
| [RFC-0005 — Git](0005-backend-git.md) | Contrato objetivo; NP-017/018 pendientes |
| [RFC-0008 — IPC Desktop](0008-contrato-ipc-desktop.md) | Contrato interno implementado |
| [RFC-0009 — CLI](0009-contrato-cli.md) | Una petición por llamada, implementada |
| [RFC-0010 — Rendimiento](0010-presupuestos-rendimiento.md) | Presupuestos y protocolos; aceptación limitada de NP-013 |
| [Importaciones](../import-contract.md) | Contrato objetivo común; NP-019/020/021 pendientes |

Cada cambio de comportamiento debe actualizar el contrato y sus consumidores
en la misma tarea SDD. Un cambio incompatible necesita decisión, fixtures y
migración explícitos antes de código. Este corte documental no cambia versiones,
DTOs, comandos ni archivos de colección; no requiere migración.

RFC-0006/0007/0011/0012 siguen reservados para sandbox, secretos, plugins y
protocolo del agente. Su aparición en la visión no significa que existan
documentos aprobados.
