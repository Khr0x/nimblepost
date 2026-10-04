---
"@nimblepost/desktop": patch
---

Block collection tree interaction with a single inert container during loading and HTTP execution, avoiding thousands of disabled button updates. Remove an unused checkbox ancestor-disabled style and use explicit selected-row classes instead of descendant :has selectors while preserving request actions and read-only controls.
