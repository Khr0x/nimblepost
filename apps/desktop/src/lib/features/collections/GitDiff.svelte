<script module lang="ts">
  import { defaultKeymap } from '@codemirror/commands';
  import { Compartment, EditorState, Text } from '@codemirror/state';
  import { Decoration, EditorView, GutterMarker, gutter, keymap } from '@codemirror/view';
  import { diffKind, type indexGitDiff } from './git-diff';

  type Diff = ReturnType<typeof indexGitDiff>;

  class DiffNumber extends GutterMarker {
    readonly value: string;

    constructor(value: string) {
      super();
      this.value = value;
    }
    eq(other: DiffNumber) { return this.value === other.value; }
    toDOM() { return document.createTextNode(this.value); }
  }

  function numbers(values: Float64Array, digits: number) {
    return gutter({
      class: 'git-diff-numbers',
      initialSpacer: () => new DiffNumber('9'.repeat(digits)),
      lineMarker: (view, block) => {
        const number = values[view.state.doc.lineAt(block.from).number - 1];
        return number ? new DiffNumber(String(number)) : null;
      },
    });
  }

  const lineStyles: Partial<Record<number, Decoration>> = {
    [diffKind.metadata]: Decoration.line({ class: 'git-diff-metadata' }),
    [diffKind.addition]: Decoration.line({ class: 'git-diff-addition' }),
    [diffKind.deletion]: Decoration.line({ class: 'git-diff-deletion' }),
  };

  const theme = EditorView.theme({
    '&': { height: '100%', color: 'var(--text)', backgroundColor: 'var(--workbench)', fontSize: '12px' },
    '&.cm-focused': { outline: 'none' },
    '&.cm-focused:has(.cm-content:focus-visible)': { outline: '1px solid var(--text-muted)', outlineOffset: '-2px' },
    '.cm-scroller': { overflow: 'auto', fontFamily: 'ui-monospace, SFMono-Regular, Consolas, monospace', lineHeight: '1.65' },
    '.cm-content': { padding: '6px 0' },
    '.cm-line': { padding: '0 12px' },
    '.cm-gutters': { backgroundColor: 'var(--sidebar)', color: 'var(--text-muted)', borderColor: 'var(--border)' },
    '.git-diff-numbers .cm-gutterElement': { minWidth: '34px', padding: '0 6px', textAlign: 'right' },
    '.git-diff-numbers + .git-diff-numbers': { borderInlineStart: '1px solid var(--border)' },
    '.git-diff-metadata': { color: 'var(--text-secondary)' },
    '.git-diff-addition': { color: 'var(--success)', backgroundColor: 'color-mix(in srgb, var(--success) 15%, var(--workbench))' },
    '.git-diff-deletion': { color: 'var(--danger)', backgroundColor: 'color-mix(in srgb, var(--danger) 14%, var(--workbench))' },
  });

  function diffMarkers({ before, after, kinds, digits }: Diff) {
    return [
      numbers(before, digits),
      numbers(after, digits),
      EditorView.decorations.of(view => {
        const { doc } = view.state;
        const first = doc.lineAt(view.viewport.from).number;
        const last = doc.lineAt(view.viewport.to).number;
        const ranges = [];
        for (let row = first; row <= last; row++) {
          const decoration = lineStyles[kinds[row - 1]];
          if (decoration) ranges.push(decoration.range(doc.line(row).from));
        }
        return Decoration.set(ranges);
      }),
    ];
  }

  function sameMarkers(previous: Diff, next: Diff) {
    return previous.kinds.length === next.kinds.length && previous.kinds.every((kind, row) =>
      kind === next.kinds[row] && previous.before[row] === next.before[row] && previous.after[row] === next.after[row]);
  }

  function changedLines(previous: Diff, next: Diff, oldDoc: Text, newDoc: Text, anchor: number) {
    // Keep the shared lines in the document so CodeMirror can map its scroll anchor.
    const shared = Math.min(oldDoc.lines, newDoc.lines);
    let prefix = 0;
    while (prefix < shared && previous.lines[prefix] === next.lines[prefix]) prefix++;
    let suffix = 0;
    while (prefix + suffix < shared &&
      previous.lines[oldDoc.lines - 1 - suffix] === next.lines[newDoc.lines - 1 - suffix]) suffix++;
    const from = prefix ? oldDoc.line(prefix).to : 0;
    const to = suffix ? oldDoc.line(oldDoc.lines - suffix + 1).from : oldDoc.length;
    const end = suffix ? newDoc.line(newDoc.lines - suffix + 1).from : newDoc.length;

    if (anchor >= from && anchor < to) {
      const oldLine = oldDoc.lineAt(anchor);
      const row = oldLine.number - 1;
      const before = previous.before[row], after = previous.after[row];
      const match = next.lines.findIndex((line, index) => line === oldLine.text &&
        ((after !== 0 && after === next.after[index]) || (before !== 0 && before === next.before[index])));
      if (match >= prefix && match < newDoc.lines - suffix) {
        const newLine = newDoc.line(match + 1);
        return [
          { from, to: oldLine.from, insert: newDoc.slice(from, newLine.from) },
          { from: oldLine.to, to, insert: newDoc.slice(newLine.to, end) },
        ];
      }
    }
    return { from, to, insert: newDoc.slice(from, end) };
  }

  function renderDiff(node: HTMLDivElement, diff: Diff) {
    const markers = new Compartment();
    const view = new EditorView({ parent: node, state: EditorState.create({
      doc: Text.of(diff.lines),
      extensions: [
        EditorState.lineSeparator.of('\n'),
        EditorState.readOnly.of(true),
        EditorView.editable.of(false),
        EditorView.contentAttributes.of({
          tabindex: '0', role: 'textbox', 'aria-multiline': 'true', 'aria-readonly': 'true',
          'aria-label': 'Diff contents with original and new line numbers',
        }),
        keymap.of(defaultKeymap),
        markers.of(diffMarkers(diff)),
        theme,
      ],
    }) });
    let previous = diff;
    return {
      update(next: Diff) {
        const oldDoc = view.state.doc;
        const newDoc = Text.of(next.lines);
        const snapshot = oldDoc.eq(newDoc) ? undefined : view.scrollSnapshot();
        const changes = snapshot && view.state.changes(changedLines(previous, next, oldDoc, newDoc, snapshot.value.range.from));
        const anchor = changes && snapshot.map(changes);
        const decorations = sameMarkers(previous, next) ? undefined : markers.reconfigure(diffMarkers(next));
        const effects = [anchor, decorations].filter(effect => effect !== undefined);
        if (changes || effects.length) view.dispatch({ changes, effects });
        previous = next;
      },
      destroy: () => view.destroy(),
    };
  }
</script>

<script lang="ts">
  let { diff }: { diff: Diff } = $props();
</script>

<div class="git-diff-editor" use:renderDiff={diff}></div>

<style>
  .git-diff-editor { flex: 1; min-width: 0; min-height: 0; overflow: hidden; }
</style>
