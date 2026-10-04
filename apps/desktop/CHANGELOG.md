# @nimblepost/desktop

## 0.2.0-alpha.1

### Minor Changes

- abed989: Add Delete to request actions, with confirmation, safe file removal, and updates to the collection tree and open tabs.
- abed989: Watch workspace collections for external file changes, refresh trees and clean requests, and protect edited drafts with reload or save-copy actions.
- abed989: Restore request tabs and unsaved drafts after restarting. Keep backups private and local, protect the final window close, and recover missing or changed files as unsaved copies.

### Patch Changes

- 37118a2: Keep body editor variable forms and completion menus outside clipped containers so they remain visible near the top edge. Preserve keyboard focus and variable editing in the detached hover form.
- abed989: Release closed response bodies, clear stale tab responses when resending, bound request label caches, and coalesce pending recovery backups during slow writes. Add a repeatable workload for many tabs, large responses and prolonged navigation.
- abed989: Build collection trees with a Map index instead of repeated sibling searches. Reuse each collection's tree when request and folder paths are unchanged, and release cached trees when their collection leaves the workspace.
- 37118a2: Organize environment management with a searchable sidebar containing only the active collection's environments and an adjacent variable editor. Add variable descriptions while preserving automatic rows, secret declarations, and Save/Reset protection for pending edits.
- abed989: Use consistent 16-pixel indentation in the collection tree and draw subtle ancestor guides with CSS on visible virtual rows, without adding DOM nodes or scroll handlers.
- abed989: Combine environment selection, creation and editing in one dropdown, matching the workspace selector. Focus the new environment name when creating and return focus to the selector when closing the editor.
  
  Match the workspace creation form with the same compact dialog, icon-only close control, Name field, and Cancel/Create actions. Limit the form to the new name, select the created environment and close the form. Runtime base URL overrides and existing variables remain in the environment editor.
  
  Open environment management as a separate main-area page, preserving request drafts on return. Reuse request-style variable rows that append an empty row while typing, with enable/remove controls and environment secret declarations. Protect pending edits when leaving or switching environments.
- abed989: Load request names and methods only for the visible tree rows and their scroll margin. Reuse cached labels, invalidate affected collections on external changes, and discard outdated reads without changing request drafts.
- abed989: Load workspace management and response inspection only when needed, reuse their modules on subsequent visits, and preserve the editor and raw response data if a panel cannot be loaded.
- abed989: Add Windows/Linux native CI smoke checks and NSIS/Debian packaging configurations. Correct release notes to describe draft recovery.
- abed989: Block collection tree interaction with a single inert container during loading and HTTP execution, avoiding thousands of disabled button updates. Remove an unused checkbox ancestor-disabled style and use explicit selected-row classes instead of descendant :has selectors while preserving request actions and read-only controls.
- 37118a2: Reorder Params, Headers, and Variables rows using drag handles or keyboard arrows. Preserve row metadata, disabled states, and secret declarations, and save the new ordering through the existing draft flow.
  
  Use pointer capture for row dragging so reordering also works inside the desktop WebView without relying on HTML5 drag-and-drop events.
- abed989: Add a CodeMirror body editor loaded on demand for JSON, XML and text, with line numbers, syntax highlighting, indentation, search/replace, undo/redo and explicit JSON formatting. Automatically close JSON brackets and quotes, with pair deletion and closing-character skipping. Preserve request drafts, use a neutral border without an active-line highlight, keep selected syntax readable in dark and light themes, and provide a plain-text fallback if the editor cannot load.
  
  Complete `{{variables}}` with a filtered menu and scope information, highlight placeholders, and edit values from a hover panel. Inherited values become request overrides through the existing Save/Discard flow; secrets remain hidden and are replaced only in memory.
  
  Open variable suggestions when typing or pasting a prefix, including an Add variable action for empty or unmatched catalogs and a visible explanation if inherited variables could not load.
- 37118a2: Show enabled query params automatically in the request URL, updating it when rows are edited, disabled, or removed. Synchronize URL edits back to query params while keeping disabled rows and avoiding duplicate params when sending.
- 37118a2: Autocomplete environment and request variables in the request URL when typing an unfinished {{placeholder, with keyboard and mouse selection. Load the shared variable context without requiring the Body tab to be opened.
  
  Highlight complete {{variable}} placeholders in green within the request URL while preserving native input editing and horizontal scrolling.
  
  Keep URL text selection consistent with Params and Headers, including when selecting part of a highlighted variable.
  
  Share variable highlighting across URL, Params, Headers, Variables, and Auth inputs. Variables appear green and selected text uses the normal foreground color. Auth placeholders are visible while literal credentials remain masked.
  
  Draw a visible selection background in the text overlay and synchronize native selection changes, including collapsing the selection with the keyboard or mouse, so variables immediately recover their green color.
  
  Show a hover card for variables in request inputs with their name, scope, preview value, and copy button. Keep secrets masked, and support opening the card with Alt+Down and dismissing it with Escape.
  
  Use NimblePost's theme colors in the hover card and allow editing variable values through the shared request override flow, with Apply and Save for regular values and memory-only updates for secrets.
- abed989: Reuse the collection tree's visible rows while scrolling instead of rebuilding thousands of row components. Restore menu and dialog focus by request identity so recycled buttons cannot target a different request. Add an isolated six-sweep native memory diagnostic.
- abed989: Remove gray backgrounds from collection and folder chevron controls in the sidebar, including hover and expanded states.
- abed989: Remove the outline around collection tree rows selected with the mouse while preserving visible focus during keyboard navigation.
- abed989: Render only the visible lines of large multiline responses, including the panel fallback. Preserve full-body copying, chunk loading and line wrapping; keep small JSON responses highlighted and foldable.
- abed989: Render only visible collection tree rows and a small margin. Closed collections and folders omit their descendants, while keyboard navigation and restored request tabs reveal their target rows.

## 0.1.1-alpha.0

### Patch Changes

- c06b7ec: Automate macOS ALPHA releases with Changesets version PRs, synchronized desktop
  and Rust versions, and verified universal installers with ad-hoc signing.
