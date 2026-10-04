---
"@nimblepost/desktop": patch
---

Reorder Params, Headers, and Variables rows using drag handles or keyboard arrows. Preserve row metadata, disabled states, and secret declarations, and save the new ordering through the existing draft flow.

Use pointer capture for row dragging so reordering also works inside the desktop WebView without relying on HTML5 drag-and-drop events.
