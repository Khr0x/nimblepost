# Desktop with Tauri 2 and Svelte 5

The ALPHA desktop client supports HTTP requests, local workspaces, multiple
collections, tabbed editing, and collection and folder defaults. The current
version is in [package.json](package.json).

The UI lives in `src/lib/features`, the IPC bridge in `src/lib/desktop`,
shared components in `src/lib/components`, and styles in `src/styles`.
Local unit tests live in `tests/unit`.

## Interface and components

The app opens in dark mode. The **Theme** selector switches between Dark and
Light and stores the choice locally across sessions. The **Zinc & Emerald**
palette uses `#09090b` for the background, `#18181b` for cards, `#27272a`
for borders, `#10b981` for the accent, `#fafafa` for text, and `#a1a1aa` for
secondary text. Emerald marks primary actions, focus, and success. HTTP
methods have their own colors, and errors use red. Light mode adjusts the
Zinc grays and Emerald contrast. The native window follows the selected theme.

Request controls use **shadcn-svelte** Button, Input, Textarea, Tabs, Select,
Checkbox, Table, Breadcrumb, Dialog, Collapsible, and ContextMenu. They cover
the method, body, auth, environments, Params/Headers/Variables rows, request
creation and saving, tab closing, and foldable JSON. Icons use **Lucide**
(`@lucide/svelte`). The components live in `src/lib/components/ui` and use
Tailwind CSS 4.

Reuse the shared components when adding request controls, or add the
corresponding component from shadcn-svelte. Keep theme tokens and HTTP method
colors consistent. Folder pickers use the native system dialog. Run the
command below to add a component from the official registry.

```sh
cd apps/desktop
npx shadcn-svelte@1.7.0 add <component>
```

## Build and run

Install Node.js 24 or later, npm, Rust, and the
[native Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).
Then run the development commands from the repository root.

```sh
npm ci
npm run desktop:dev
```

The native executable, product, and window title all use the name
**NimblePost**. On macOS, install the Xcode Command Line Tools. Use the
following command to compile just the executable.

```sh
npm run desktop:build
```

## Collections, requests, and workspaces

Choose **New collection**, name it, and select its location. Choose
**New request** in that collection, enter a name, add a URL, and click
**Send**. The app creates a new directory under the location you select
and opens the new request. A new request requires only a name, defaults to
GET, and derives its `.yml` filename from its name. You can add the URL
later. The selected tree folder determines the destination. Directory names
for collections and folders are suggested from their display names and
can be edited. Existing destinations are protected. Bundled examples are
read-only. Request URLs may contain variables such as `{{baseUrl}}/users`.

Use **Open collection** to select the folder containing `opencollection.yml`.
Open a request, click **Send**, and inspect **Body** or **Headers**.
Rust indexes the tree and reads request files as you select them.

Use the **Workspace** menu in the navigation bar to switch between local
workspaces or choose **Create workspace** or **Manage workspaces**.
**My Workspace** is the default. A check and the Emerald accent identify
the active workspace. Home returns to the start page after you save or
discard pending edits.

**Manage workspaces** opens in the main area and lets you rename or delete
active and inactive workspaces. Its module loads on first use and is
reused. If it fails to load, the editor and drafts stay available.
**Back to workspace** returns to the editor with the tabs and pending
edits intact. Renaming a workspace preserves its selection, files, and
editor response. Deleting a workspace asks for confirmation and removes its
record and references. Deleting the active workspace returns to the default
one, including any new name you gave it. Deleting an inactive workspace
keeps the current selection. The app creates an empty **My Workspace** if
the default is missing, including after you delete the last workspace.

Each workspace holds several independent collection trees and one active
collection for editing and sending. **New collection** and **Open collection**
add to the current workspace. Opening the same folder again reuses its
reference. **Remove** removes a reference from the workspace and keeps
the files. A reference to a moved or inaccessible folder remains visible
with a warning so you can remove it and open the new location.

Tauri restores names, folder references, read-only access, and the active
selection from `workspaces.json` in the app's local data directory. The
current format is version 2. It loads version 1 and migrates the original
workspace label while retaining references. The app writes the file through
a temporary file with mode 0600 on Unix. It stores references and selections
outside the collections. Requests, responses, secrets, and drafts are kept
out of this file. A corrupt or externally changed file is retained and
blocks writes to the registry. One app instance writes the registry.
Workspace variables and cross-device synchronization are outside the
current scope.

To try the bundled example, start `node examples/basic-http/server.mjs`
from the repository root and choose **Try example**. The app selects
`users/list.yml` and the `local` environment. The
[CLI](../../bins/cli/README.md) can run the same request.

## Tabs and recovery

The workbench **+** button creates an **Untitled** request in memory.
On its first **Save**, select a name, collection, and folder. Saved requests
use their existing file. Double-click the name in the breadcrumb to rename
it. Tabs are capped at 240 px and truncate long names. Chevrons scroll
overflowing tabs while **+** stays visible. Opening or selecting a request
scrolls its tab into view.

A new, unedited tab closes immediately. Closing an edited request offers
**Save**, **Close without saving**, and **Cancel**. Saving an untitled request
opens the destination picker. A failed or cancelled save preserves the tab
and the draft. Restarting restores the active workspace's tabs, their order,
the active tab, environment selections, and drafts, including **Untitled**
requests. The app reloads the revision of each file on open. If the file
changed or disappeared, the recovered request becomes an unsaved copy.
**Save** creates a complete copy at a new location and preserves fields the
editor cannot edit.

After 400 ms without changes, the app writes a synced temporary file and
replaces `recovery.json` outside the collections. On Unix the file uses
mode 0600. It captures request configuration, including bodies and literal
values. Use placeholders for credentials. Responses, runtime secrets,
the base URL override, and environment drafts are excluded. The backup is
limited to 128 tabs and 16 MiB. A corrupt, incompatible, symlinked, or
externally changed backup is preserved and the app shows a warning. One
instance writes the backup. A forced exit restores the last completed
backup. If a write is slow, the app keeps the current write and the latest
pending backup. Closing waits for the final backup and keeps the window
open on failure when edited drafts remain.

Open tabs keep their loaded responses. Resending drops the previous
response body from the tab. Closing a response frees its Rust data if the
response ID still matches. Closing an older response preserves the newer one.

**Delete** in the request tree menu deletes a request file at the collection
root or in a folder and closes its tab. The confirmation also identifies
unsaved edits that would be lost. Cancelling or a failed delete keeps the
request and its edits. Deletion respects read-only access and blocks files
changed since the opened revision. It preserves folders and other requests.

## Editing and sending

Use **Params**, **Auth**, **Headers**, **Body**, and **Variables** to edit a
request. The app preserves duplicates and enabled flags. Request Params,
Headers, and Variables keep a blank final row and add another as you type.
That placeholder is excluded from saves and does not mark a draft as edited.
The client supports JSON, text, and XML bodies and Basic, Bearer, API Key,
and Inherit authentication.

Opening a supported body type loads CodeMirror 6 with line numbers, JSON/XML
highlighting, two-space indentation, find/replace, and undo/redo. JSON
automatically pairs braces, brackets, and quotes, places the cursor between
them, and skips a closing character that is already present. Backspace
inside an empty pair removes both characters. The editor uses a neutral
border and leaves the active line unhighlighted. Use Ctrl/Cmd+F to find and
Ctrl/Cmd+[ or ] to adjust indentation. Tab moves between controls unless
it accepts a suggestion. The app mounts one editor instance. Switching
requests or leaving **Body** retains the draft and releases that view's
undo history. **Discard** also resets undo history.

**Format JSON** is explicit and undoable. It preserves large numbers,
duplicate keys, and escape sequences. Invalid JSON and text outside its
limits are left intact. Opening, saving, and changing body types preserve
formatting, Unicode, and CRLF/CR/LF line endings. Pasted line breaks follow
the document's convention. If CodeMirror fails to load, an editable text
area and **Retry editor** remain available with the draft intact.

Typing or pasting a prefix starting with `{{` opens variable suggestions.
Use arrows, Enter, or Tab to choose an item and complete the placeholder
without repeating the braces. If there are no matches, the menu offers to
add a variable in **Variables**. A failure to load inherited context is
shown in the menu. Suggestions show the source scope and file. Precedence,
lowest first, is collection, folders, environment, request, and runtime
override. Defined placeholders appear purple and missing ones appear red.

Hover over a placeholder or press Alt+Enter with the cursor inside it to
edit the value. **Apply** edits the request variable or creates a request
override for an inherited variable. **Save** writes the draft and
**Discard** drops it. The `baseUrl` override is transient and takes
precedence over YAML. Secrets use an empty password field for in-memory
replacement. Their values stay out of the display, YAML, and history.

The **Environment** menu selects an environment or opens **Create environment**
or **Manage environments**. Creation needs only a name and selects the new
environment. Management opens a separate view with a searchable sidebar for
the active collection's environments. Returning keeps the current request.
The table edits names, values, and descriptions and appends a blank row as
you type. Rows can be enabled, edited, deleted, or declared secret. The
selected environment is marked in the sidebar. **Save** and **Reset**
handle pending edits before you leave or select another environment.

**Send** or Ctrl/Cmd+Enter runs the draft through the Rust core.
**Cancel** cancels it. The response panel shows status, duration, size, and
**Body** and **Headers**. Drag its divider to resize it. The bounds leave
room for the editor and the app stores the height locally. The panel and
its JSON tools load with the first response and are reused. If loading fails,
the raw body, headers, HTTP metrics, and subsequent chunks stay available.

A valid complete JSON body defaults to color-coded **Pretty** with collapsible
objects and arrays. **Text**, **Raw**, wrapping, and copying are also
available. Binary responses use **Hex**. Formatting follows size and depth
limits and preserves the original numeric values. Copy, save, and reload show
a temporary check icon. Rust retains the body and sends it in 64 KiB chunks.
Load more chunks manually up to the 16 MiB limit. The backend retains the
most recent response for a tab, and full copying requires the complete body.

## Saving and external changes

Tauri commands run in a session scoped to a root the user selects.
JavaScript uses the native backend for filesystem and HTTP operations.
HTML responses appear as text. **Save** or Ctrl/Cmd+S writes edited fields.
**Discard** reloads from disk or resets an untitled request. Switching tabs
retains the draft. Save or discard pending changes before changing workspace
or leaving a collection. Closing the window waits for the newest recovery
backup. A backup failure blocks closing while edited drafts remain, and
pending environment edits require **Save** or **Discard**. Unedited new tabs
close without blocking. The bundled example is read-only. To edit the
original files, open `examples/basic-http` with the folder picker.

The backend keeps the original bytes and concrete syntax tree, compares the
revision, validates the result, syncs a temporary file in the same directory,
and replaces the original. A no-op save leaves bytes and modification time
intact. Saves preserve permissions, comments, unknown fields, and formatting
in untouched areas. New nodes can use flow YAML. An external change or
deletion blocks saving and requires a reload. If a conflict is detected after
a temporary file was written, the app reports its path for recovery. External
writers can race between the final comparison and rename.

Each send reloads defaults and the environment. A draft whose file revision
differs from the one on disk is blocked. The current scope is single-file
saves and filesystem collections.

The native `notify` watcher monitors collections in the active workspace and
their parents for creations, changes, moves, deletions, and root replacements
from an editor or Git. Like the tree index, it skips symlinks, hidden paths,
`.git`, `node_modules`, `scripts`, `fixtures`, and `auth`. It groups
notifications after 300 ms of inactivity. Refresh waits during HTTP execution,
saving, renaming, and open dialogs. Switching workspaces drops the previous
subscriptions.

A tree refresh keeps the active collection and its revisions. Clean tabs
reload, and edited tabs retain their drafts even when inactive. Any external
change, including comment-only edits, offers **Reload from disk** or
**Save copy** and blocks **Save** and **Send** until resolved.
**Reload from disk** discards the selected draft. **Save copy** preserves its
complete configuration in an unsaved request and opens the destination picker.
Deleting or renaming a file externally automatically retains an unsaved copy.
An unreadable YAML file or inaccessible collection keeps the open content
visible and displays a warning. Correct the file to recover normal operation.

**Refresh collections** and refocusing the window reread the workspace.
Use them when filesystem notifications are unreliable or the watcher reports
an error. Native watcher tests run on macOS and in the Windows and Linux
validation jobs.

## MVP 0 secrets and history policy

Supply **Secrets · memory only** values for **Send**. They clear when you
change environment or collection and stay out of YAML and history. The CLI
reads only process variables explicitly named with `--secret-env`. **Save**
writes templates and edited configuration values, including the configured
body. Use `{{token}}` or `{{password}}` to keep credentials out of files.

**History** stores UTC timestamps, relative request paths, methods,
status or result categories, durations, and received byte counts. It excludes
URLs, resolved queries, environment values, credentials, headers, error
messages, and request and response bodies. Method names are standardized or
stored as CUSTOM. The relative path identifies the file without revealing
the collection root.

The app keeps the last 200 executions in `history.json` under the Tauri
data directory (`dev.nimblepost.desktop`), with atomic replacement and
mode 0600 on Unix. **Clear history** clears the entries. An incompatible
or corrupt file is retained and reported. History write failures preserve
the HTTP result. One app instance writes the history file. Storing
bodies, headers, or credentials would require a separate decision with explicit
consent and rules for redaction, retention, and deletion. A persistent
secret store and keychain integration are outside the current scope.

```sh
npm run check
npm run build
cargo test --workspace --locked
```

## Tree and response rendering

The tree mounts rows in the viewport plus six extra at each end. Scrolling
reuses those rows and their components. Paths, permissions, labels, and
accessibility attributes update with the content. Menu and dialog focus
tracks the logical request even when its button was reused off-screen.
Collapsed collections and folders omit their descendants. Selecting a tab
reveals its request and expands its folders. Expansion survives scrolling
and refreshes. The tree takes a single Tab stop. Use arrows to move or
expand, Enter to open a request, or Shift+F10 for actions.

Request names and methods load only for mounted rows and are reused on
return. Each collection caches labels and read attempts for up to 512 paths,
or the visible row count if it is higher. Evicted rows are read again when they
appear, and failed attempts may also be evicted. While loading, the tree shows
the filename and `HTTP`. An external notification invalidates the affected
collection's labels. Visible labels are refreshed and hidden ones are refreshed
when they appear. Other collections keep their caches. Invalid YAML or an
unreadable file keeps the filename visible without continuous retries.
Use **Refresh collections** to try again. The active request label reflects
its draft.

Responses over 500 lines mount the visible lines plus six extra at each end.
The index stores offsets into the original body and measures wrapped line
heights as they appear. **Copy response body** accesses the complete loaded
body. Pretty keeps its formatting and values in large responses and omits
highlighting and collapsing beyond that threshold. Smaller responses retain
those features. The fallback viewer uses the same line limit. Native text
selection and search are limited to mounted text. The line optimization
does not shorten an individual long line.

## Native packaging and performance

`scripts/native-performance.mjs` and `scripts/performance-report.mjs`
measure and compare ordinary startup and resident memory on Linux with
isolated local profiles. These measurements cover ordinary startup, with
cold starts and absolute targets measured separately.

`tauri.windows.conf.json` configures NSIS and `tauri.linux.conf.json`
configures Debian. The manual **Native distribution validation** workflow
builds, installs, launches, and uninstalls Windows and Linux x64 packages.
Interactive validation passed on Windows 11 x64 and Debian 13 x86_64 for
the packages from [run 44791755659](https://github.com/Khr0x/nimblepost/actions/runs/44791755659).
Public ALPHA installers are distributed for macOS.

The manual **Native performance tooling** workflow exercises large
responses, clipboard operations, cancellation, retention, and interactions
on Linux. Native package and performance validation run separately from
regular CI.

## Collection and folder defaults

Select **Edit defaults** in a collection or folder menu to edit scoped
headers, authentication, and variables, with source information.
**Save** and **Discard** are explicit. Opening the editor leaves disk untouched,
and a missing `folder.yml` is created exclusively on save. Conflicts and
read-only status block an overwrite. The dialog protects pending edits and
supports keyboard navigation and focus. The request recovery file excludes
defaults drafts.
