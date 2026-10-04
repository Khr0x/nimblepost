---
"@nimblepost/desktop": patch
---

Build collection trees with a Map index instead of repeated sibling searches. Reuse each collection's tree when request and folder paths are unchanged, and release cached trees when their collection leaves the workspace.
