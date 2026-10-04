<script lang="ts">
  import { onMount } from 'svelte';
  import { Compartment, EditorState, Prec, Transaction } from '@codemirror/state';
  import { Decoration, EditorView, MatchDecorator, ViewPlugin, activateHover, closeHoverTooltips, drawSelection, hoverTooltip, keymap, lineNumbers } from '@codemirror/view';
  import { acceptCompletion, autocompletion, closeBrackets, closeBracketsKeymap, pickedCompletion, startCompletion, type CompletionContext } from '@codemirror/autocomplete';
  import type { VariablePreview } from '$lib/desktop/api';
  import { defaultKeymap, history, historyKeymap, isolateHistory, redo, redoDepth, undo, undoDepth } from '@codemirror/commands';
  import { HighlightStyle, bracketMatching, indentOnInput, indentUnit, syntaxHighlighting } from '@codemirror/language';
  import { openSearchPanel, search, searchKeymap } from '@codemirror/search';
  import { json } from '@codemirror/lang-json';
  import { xml } from '@codemirror/lang-xml';
  import { tags } from '@lezer/highlight';
  import { formatJSON } from '../responses/response-body';
  import { Button } from '$lib/components/ui/button';
  import { Redo2, Search, Undo2 } from '@lucide/svelte';

  let { value, bodyType, disabled = false, onchange, variables = [], providedSecrets = [], onvariableedit, onaddvariable,
    variableIssue = '', variablesLoading = false }: {
    value: string; bodyType: string; disabled?: boolean; onchange: (value: string) => void;
    variables?: VariablePreview[]; providedSecrets?: string[]; onvariableedit: (name: string, value: string) => void;
    onaddvariable: (name: string) => void; variableIssue?: string; variablesLoading?: boolean;
  } = $props();
  let host: HTMLDivElement;
  let editor = $state.raw<EditorView | null>(null);
  let canUndo = $state(false), canRedo = $state(false);
  let formatIssue = $state('');
  let completionFrame = 0;
  const language = new Compartment(), access = new Compartment(), variableSupport = new Compartment();
  const variableNames = $derived(new Map(variables.map(variable => [variable.name, variable])));
  const variablePattern = () => /\{\{[ \t]*([\w.-]+)[ \t]*\}\}/g;
  const origin = (variable: VariablePreview | undefined) => variable
    ? `${variable.scope}${variable.path ? ` · ${variable.path.split(/[\\/]/).slice(-2).join('/')}` : ''}${variable.secret ? ' · secret' : ''}` : 'Not defined';
  function insertVariable(view: EditorView, completion: { label: string }, start: number, end: number) {
    if (view.state.readOnly) return;
    // Replace an existing suffix/closing pair, including JSON's automatic braces.
    const suffix = view.state.sliceDoc(end, view.state.doc.lineAt(end).to).match(/^[\w.-]*[ \t]*\}{0,2}/)?.[0] ?? '';
    const insert = `${completion.label}}}`;
    view.dispatch({ changes: { from: start, to: end + suffix.length, insert }, selection: { anchor: start + insert.length },
      annotations: [pickedCompletion.of(completion), Transaction.userEvent.of('input.complete')] });
  }
  function completeVariable(context: CompletionContext, status: string) {
    if (context.state.readOnly) return null;
    const match = context.matchBefore(/\{\{[ \t]*[\w.-]*$/);
    if (!match) return null;
    const from = match.from + 2 + (match.text.slice(2).match(/^[ \t]*/)?.[0].length ?? 0);
    const prefix = context.state.sliceDoc(from, context.pos);
    const hasMatch = (text: string) => variables.some(variable => variable.name.toLowerCase().startsWith(text.toLowerCase()));
    if (!hasMatch(prefix)) return { from, filter: false, options: [{
      label: prefix || 'Add variable', displayLabel: prefix ? `Add variable ${prefix}` : 'Add a request variable…',
      detail: status || (variables.length ? 'No matching variable' : 'No variables defined'),
      apply(view: EditorView, _completion: unknown, start: number, end: number) {
        if (view.state.readOnly) return;
        if (prefix) insertVariable(view, { label: prefix }, start, end);
        onaddvariable(prefix);
      },
    }] };
    return { from, validFor: (text: string) => /^[\w.-]*$/.test(text) && hasMatch(text), options: variables.map(variable => ({
      label: variable.name, type: 'variable', detail: origin(variable),
      apply: insertVariable,
    })) };
  }
  const variableHover = hoverTooltip((view, pos, side) => {
    const line = view.state.doc.lineAt(pos);
    for (const match of line.text.matchAll(variablePattern())) {
      const from = line.from + match.index!, to = from + match[0].length;
      if (pos < from || pos > to || pos === from && side < 0 || pos === to && side > 0) continue;
      const name = match[1];
      return { pos: from, end: to, above: true, arrow: true, create() {
        const variable = variableNames.get(name), secret = variable?.secret ?? false;
        const dom = document.createElement('form'); dom.className = 'cm-variable-hover';
        dom.setAttribute('aria-label', `Variable ${name}`);
        const title = document.createElement('strong'); title.textContent = `{{${name}}}`;
        const source = document.createElement('small'); source.textContent = origin(variable);
        const label = document.createElement('label'); label.textContent = secret ? 'New secret value' : 'Value';
        const input = !secret && /[\r\n]/.test(variable?.value ?? '') ? document.createElement('textarea') : document.createElement('input');
        if (input instanceof HTMLInputElement) input.type = secret ? 'password' : 'text';
        input.setAttribute('aria-label', `Value of ${name}`); input.autocomplete = 'off'; input.spellcheck = false;
        input.value = secret ? '' : variable?.value ?? ''; label.append(input);
        const initialValue = input.value;
        const note = document.createElement('small'); note.setAttribute('role', 'status');
        note.textContent = secret ? `${variable?.scope === 'runtime' || providedSecrets.includes(name) ? 'Secret provided' : 'Secret not provided'} · memory only`
          : variable?.scope === 'runtime' ? 'Changes the runtime override.'
          : variable?.scope === 'request' ? 'Changes this request. Use Save to persist.'
          : 'Creates a request override. Use Save to persist.';
        const apply = document.createElement('button'); apply.type = 'submit'; apply.textContent = secret ? 'Set secret' : 'Apply';
        const syncAccess = () => { input.disabled = disabled; apply.disabled = disabled; };
        syncAccess(); dom.append(title, source, label, note, apply);
        dom.addEventListener('submit', event => {
          event.preventDefault();
          if (disabled) return;
          onvariableedit(name, !secret && input.value === initialValue ? variable?.value ?? '' : input.value);
          input.value = secret ? '' : input.value;
          view.dispatch({ effects: closeHoverTooltips }); view.focus();
        });
        dom.addEventListener('keydown', event => {
          if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); view.dispatch({ effects: closeHoverTooltips }); view.focus(); }
          if (event.key === 'Enter') event.stopPropagation();
        });
        return { dom, update: syncAccess };
      } };
    }
    return null;
  }, { hideOnChange: true });
  function variableExtensions() {
    const known = variableNames;
    const status = variableIssue ? 'Inherited variables unavailable' : variablesLoading ? 'Loading variables…' : '';
    const matcher = new MatchDecorator({ regexp: variablePattern(), decoration: match => Decoration.mark({
      class: `cm-body-variable${known.has(match[1]) ? '' : ' cm-body-variable-missing'}`,
      attributes: { 'data-variable': match[1] },
    }) });
    return [autocompletion({ override: [context => completeVariable(context, status)], filterStrict: true, activateOnTypingDelay: 50 }),
      ViewPlugin.define(view => ({ decorations: matcher.createDeco(view), update(update) {
        this.decorations = matcher.updateDeco(update, this.decorations);
      } }), { decorations: plugin => plugin.decorations })];
  }
  const languageSupport = (type: string) => type === 'json' ? [json(), closeBrackets(), Prec.high(keymap.of(closeBracketsKeymap))] : type === 'xml' ? xml() : [];
  const editing = (locked: boolean) => [EditorState.readOnly.of(locked), EditorView.editable.of(!locked),
    EditorView.contentAttributes.of({ 'aria-label': 'Request body', role: 'textbox', 'aria-multiline': 'true',
      'aria-readonly': String(locked), spellcheck: 'false', autocapitalize: 'off', autocorrect: 'off' })];
  const colors = HighlightStyle.define([
    { tag: [tags.propertyName, tags.tagName, tags.attributeName], color: 'var(--json-key)', '--cm-selection-text': 'var(--json-key)' },
    { tag: [tags.string, tags.attributeValue], color: 'var(--json-string)', '--cm-selection-text': 'var(--json-string)' },
    { tag: tags.number, color: 'var(--json-number)', '--cm-selection-text': 'var(--json-number)' },
    { tag: tags.bool, color: 'var(--json-boolean)', '--cm-selection-text': 'var(--json-boolean)' },
    { tag: tags.null, color: 'var(--json-null)', '--cm-selection-text': 'var(--json-null)' },
    { tag: [tags.comment, tags.meta], color: 'var(--text-muted)', '--cm-selection-text': 'var(--text-muted)' },
  ]);
  const theme = EditorView.theme({
    '&': { color: 'var(--text)', backgroundColor: 'var(--workbench)', fontSize: '12px' },
    '&.cm-focused': { outline: 'none' },
    '.cm-scroller': { fontFamily: 'ui-monospace, SFMono-Regular, Consolas, monospace', lineHeight: '1.6', overflow: 'auto' },
    '.cm-content': { padding: '8px 0', caretColor: 'var(--text)', '--cm-selection-text': 'var(--text)' },
    '.cm-line': { padding: '0 12px' },
    '.cm-gutters': { backgroundColor: 'var(--workbench)', color: 'var(--text-muted)', borderColor: 'var(--border)' },
    '.cm-cursor': { borderLeftColor: 'var(--text)' },
    // Match CodeMirror's focused selection specificity, including its layer.
    '&.cm-editor .cm-selectionLayer .cm-selectionBackground, &.cm-editor.cm-focused > .cm-scroller > .cm-layer.cm-selectionLayer .cm-selectionBackground': { backgroundColor: 'color-mix(in srgb, var(--brand) 25%, var(--workbench))' },
    '.cm-line::selection, .cm-line ::selection': { color: 'var(--cm-selection-text)' },
    '.cm-body-variable, .cm-body-variable *': { color: 'var(--json-boolean)', '--cm-selection-text': 'var(--json-boolean)', backgroundColor: 'color-mix(in srgb, var(--json-boolean) 12%, transparent)', borderRadius: '3px' },
    '.cm-body-variable-missing, .cm-body-variable-missing *': { color: 'var(--danger)', '--cm-selection-text': 'var(--danger)', textDecoration: 'underline dotted', backgroundColor: 'var(--danger-subtle)' },
    '.cm-tooltip': { backgroundColor: 'var(--surface)', color: 'var(--text)', border: '1px solid var(--border)', borderRadius: '6px', boxShadow: '0 4px 16px #0003' },
    '.cm-tooltip-autocomplete > ul': { fontFamily: 'ui-monospace, SFMono-Regular, Consolas, monospace', fontSize: '12px', maxWidth: 'min(500px, 80vw)' },
    '.cm-tooltip-autocomplete > ul > li': { padding: '4px 8px' },
    '.cm-tooltip-autocomplete > ul > li[aria-selected]': { backgroundColor: 'var(--subtle)', color: 'var(--text)' },
    '.cm-completionDetail': { color: 'var(--text-muted)', fontSize: '10px' },
    '.cm-variable-hover': { display: 'flex', flexDirection: 'column', gap: '6px', padding: '10px', minWidth: '240px', maxWidth: 'min(360px, 80vw)', fontFamily: 'Inter, sans-serif', fontSize: '12px' },
    '.cm-variable-hover strong': { color: 'var(--json-boolean)', overflowWrap: 'anywhere' },
    '.cm-variable-hover small': { color: 'var(--text-muted)', fontSize: '11px', overflowWrap: 'anywhere' },
    '.cm-variable-hover label': { display: 'flex', flexDirection: 'column', gap: '4px' },
    '.cm-variable-hover input, .cm-variable-hover textarea': { width: '100%', padding: '5px 7px', backgroundColor: 'var(--workbench)', color: 'var(--text)', border: '1px solid var(--border)', borderRadius: '4px', fontFamily: 'ui-monospace, SFMono-Regular, Consolas, monospace', fontSize: '12px' },
    '.cm-variable-hover textarea': { minHeight: '60px', maxHeight: '180px', resize: 'vertical' },
    '.cm-variable-hover input:focus-visible, .cm-variable-hover textarea:focus-visible, .cm-variable-hover button:focus-visible': { outline: '2px solid var(--json-boolean)', outlineOffset: '1px' },
    '.cm-variable-hover button': { alignSelf: 'flex-end', padding: '4px 10px', backgroundColor: 'var(--subtle)', color: 'var(--text)', border: '1px solid var(--border)', borderRadius: '4px', cursor: 'pointer' },
    '.cm-panels': { backgroundColor: 'var(--surface)', color: 'var(--text)', borderColor: 'var(--border)' },
    '.cm-search': { padding: '6px', fontFamily: 'Inter, sans-serif', fontSize: '11px' },
    '.cm-panel.cm-search label': { display: 'inline-flex', alignItems: 'center', gap: '4px', marginLeft: '6px', fontSize: '11px', whiteSpace: 'nowrap' },
    '.cm-search input': { accentColor: 'var(--brand)' },
    '.cm-panel.cm-search .cm-textfield': { fontSize: '11px', padding: '2px 5px', backgroundColor: 'var(--workbench)', color: 'var(--text)', border: '1px solid var(--border)', borderRadius: '4px' },
    '.cm-panel.cm-search .cm-button': { fontSize: '11px', padding: '2px 6px', backgroundImage: 'none', backgroundColor: 'var(--subtle)', color: 'var(--text)', border: '1px solid var(--border)', borderRadius: '4px' },
  });
  function createState(text: string) {
    return EditorState.create({ doc: text, extensions: [
      // Preserve CRLF and mixed endings when editing; formatting is explicit.
      EditorState.lineSeparator.of(text.includes('\r\n') && !/\r(?!\n)|(^|[^\r])\n/.test(text) ? '\r\n' : text.includes('\n') || !text.includes('\r') ? '\n' : '\r'),
      EditorView.clipboardInputFilter.of((text, state) => text.replace(/\r\n?|\n/g, state.lineBreak)),
      lineNumbers(), drawSelection(), bracketMatching(), indentOnInput(), indentUnit.of('  '),
      history(), keymap.of([...historyKeymap, ...searchKeymap, ...defaultKeymap]), search({ top: true }),
      variableSupport.of(variableExtensions()), variableHover,
      Prec.high(keymap.of([{ key: 'Tab', run: acceptCompletion }, { key: 'Alt-Enter', run: view => {
        activateHover(view, view.state.selection.main.head, 1);
        requestAnimationFrame(() => view.dom.querySelector<HTMLElement>('.cm-variable-hover input, .cm-variable-hover textarea')?.focus());
        return true;
      } }])),
      syntaxHighlighting(colors), theme, language.of(languageSupport(bodyType)), access.of(editing(disabled)),
      EditorView.updateListener.of(update => {
        canUndo = undoDepth(update.state) > 0; canRedo = redoDepth(update.state) > 0;
        if (update.docChanged && !disabled) { formatIssue = ''; onchange(update.state.sliceDoc()); }
        if (update.docChanged && !update.transactions.some(tr => tr.isUserEvent('input.complete'))) {
          cancelAnimationFrame(completionFrame);
          // Open suggestions for all text edits, including paste.
          completionFrame = requestAnimationFrame(() => {
            const view = update.view;
            if (editor !== view || !view.hasFocus || view.state.readOnly) return;
            const head = view.state.selection.main.head;
            if (/\{\{[ \t]*[\w.-]*$/.test(view.state.sliceDoc(view.state.doc.lineAt(head).from, head))) startCompletion(view);
          });
        }
      }),
    ] });
  }
  onMount(() => {
    const view = new EditorView({ state: createState(value), parent: host });
    editor = view;
    return () => { cancelAnimationFrame(completionFrame); editor = null; view.destroy(); };
  });
  $effect(() => {
    const text = value;
    if (editor && text !== editor.state.sliceDoc()) {
      // Reload/discard starts a fresh history; undo must not revive discarded data.
      editor.setState(createState(text)); canUndo = false; canRedo = false; formatIssue = '';
    }
  });
  $effect(() => { if (editor) editor.dispatch({ effects: language.reconfigure(languageSupport(bodyType)) }); });
  $effect(() => { if (editor) editor.dispatch({ effects: access.reconfigure(editing(disabled)) }); });
  $effect(() => {
    const extensions = variableExtensions();
    if (editor) {
      editor.dispatch({ effects: variableSupport.reconfigure(extensions) });
      if (editor.hasFocus && /\{\{[ \t]*[\w.-]*$/.test(editor.state.sliceDoc(editor.state.doc.lineAt(editor.state.selection.main.head).from, editor.state.selection.main.head))) startCompletion(editor);
    }
  });
  function command(action: (view: EditorView) => boolean) {
    if (editor) { action(editor); editor.focus(); }
  }
  function format() {
    if (!editor || disabled) return;
    const text = editor.state.sliceDoc(), result = formatJSON(text);
    if (result.pretty === null) { formatIssue = result.issue === 'large' ? 'This body is too large to format.' : 'This body is not valid JSON.'; return; }
    formatIssue = '';
    const pretty = result.pretty.replace(/\n/g, editor.state.lineBreak);
    if (pretty !== text) editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: pretty },
      annotations: [Transaction.userEvent.of('input.format'), isolateHistory.of('full')] });
    editor.focus();
  }
</script>

<div class="body-editor">
  <div class="body-editor-toolbar">
    <Button variant="ghost" size="icon-sm" aria-label="Undo body edit" title="Undo (⌘ / Ctrl + Z)" disabled={disabled || !canUndo} onclick={() => command(undo)}><Undo2 aria-hidden="true" /></Button>
    <Button variant="ghost" size="icon-sm" aria-label="Redo body edit" title="Redo (⌘ / Ctrl + Shift + Z)" disabled={disabled || !canRedo} onclick={() => command(redo)}><Redo2 aria-hidden="true" /></Button>
    <Button variant="ghost" size="icon-sm" aria-label="Find and replace in body" title="Find and replace (⌘ / Ctrl + F)" onclick={() => command(openSearchPanel)}><Search aria-hidden="true" /></Button>
    {#if bodyType === 'json'}<Button variant="ghost" size="sm" onclick={format} disabled={disabled || !value}>Format JSON</Button>{/if}
    <span class="body-editor-help">Variables: {'{{'} · hover / Alt + Enter</span>
  </div>
  {#if formatIssue}<p class="body-format-issue" role="alert">{formatIssue}</p>{/if}
  <div class="body-editor-content" bind:this={host}></div>
</div>

<style>
  .body-editor { width:100%; min-width:0; border:1px solid var(--border); border-radius:6px; overflow:hidden; }
  .body-editor-toolbar { display:flex; align-items:center; flex-wrap:wrap; gap:2px; padding:4px; border-bottom:1px solid var(--border); }
  .body-editor-toolbar :global(button) { color:var(--text); }
  .body-editor-help { margin-left:auto; padding:0 6px; color:var(--text-muted); font-size:10px; }
  .body-editor-content :global(.cm-editor) { height:clamp(160px,28vh,320px); }
  .body-format-issue { padding:4px 8px; color:var(--danger); }
</style>
