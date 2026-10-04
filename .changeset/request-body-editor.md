---
"@nimblepost/desktop": patch
---

Add a CodeMirror body editor loaded on demand for JSON, XML and text, with line numbers, syntax highlighting, indentation, search/replace, undo/redo and explicit JSON formatting. Automatically close JSON brackets and quotes, with pair deletion and closing-character skipping. Preserve request drafts, use a neutral border without an active-line highlight, keep selected syntax readable in dark and light themes, and provide a plain-text fallback if the editor cannot load.

Complete `{{variables}}` with a filtered menu and scope information, highlight placeholders, and edit values from a hover panel. Inherited values become request overrides through the existing Save/Discard flow; secrets remain hidden and are replaced only in memory.

Open variable suggestions when typing or pasting a prefix, including an Add variable action for empty or unmatched catalogs and a visible explanation if inherited variables could not load.
