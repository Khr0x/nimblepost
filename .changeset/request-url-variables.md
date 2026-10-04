---
"@nimblepost/desktop": patch
---

Autocomplete environment and request variables in the request URL when typing an unfinished {{placeholder, with keyboard and mouse selection. Load the shared variable context without requiring the Body tab to be opened.

Highlight complete {{variable}} placeholders in green within the request URL while preserving native input editing and horizontal scrolling.

Keep URL text selection consistent with Params and Headers, including when selecting part of a highlighted variable.

Share variable highlighting across URL, Params, Headers, Variables, and Auth inputs. Variables appear green and selected text uses the normal foreground color. Auth placeholders are visible while literal credentials remain masked.

Draw a visible selection background in the text overlay and synchronize native selection changes, including collapsing the selection with the keyboard or mouse, so variables immediately recover their green color.

Show a hover card for variables in request inputs with their name, scope, preview value, and copy button. Keep secrets masked, and support opening the card with Alt+Down and dismissing it with Escape.

Use NimblePost's theme colors in the hover card and allow editing variable values through the shared request override flow, with Apply and Save for regular values and memory-only updates for secrets.
