---
"@nimblepost/desktop": patch
---

Combine environment selection, creation and editing in one dropdown, matching the workspace selector. Focus the new environment name when creating and return focus to the selector when closing the editor.

Match the workspace creation form with the same compact dialog, icon-only close control, Name field, and Cancel/Create actions. Limit the form to the new name, select the created environment and close the form. Runtime base URL overrides and existing variables remain in the environment editor.

Open environment management as a separate main-area page, preserving request drafts on return. Reuse request-style variable rows that append an empty row while typing, with enable/remove controls and environment secret declarations. Protect pending edits when leaving or switching environments.
