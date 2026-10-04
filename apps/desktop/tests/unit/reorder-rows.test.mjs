import assert from 'node:assert/strict';
import { test } from 'node:test';
import { reorderRows } from '../../src/lib/components/reorder-rows.ts';
import { draftFrom, editDraft } from '../../src/lib/features/requests/draft.ts';
import { requestUrlWithParams } from '../../src/lib/features/requests/request-url.ts';

test('reordering preserves whole rows, moves in both directions, and does not mutate the original', () => {
  const rows = [{name:'first',value:'1'}, {name:'off',disabled:true,description:'Keep me'}, {name:'token',secret:true,value:{type:'string',data:'{{secret}}'},foreign:{keep:true}}];
  const moved = reorderRows(rows, 0, 2);
  assert.deepEqual(moved.map(row => row.name), ['off', 'token', 'first']);
  assert.deepEqual(reorderRows(moved, 2, 0), rows);
  assert.deepEqual(rows.map(row => row.name), ['first', 'off', 'token']);
  assert.equal(moved[1], rows[2]);
  for (const [from,to] of [[0,0],[-1,1],[0,3],[3,0],[0,0.5]]) assert.equal(reorderRows(rows,from,to),rows);
  assert.deepEqual(reorderRows([],0,1),[]);
});

test('reordering is recorded as a draft edit and changes query ordering without enabling disabled rows', () => {
  const draft = draftFrom({revision:1, diagnostics:[], document:{http:{url:'/users',params:[
    {name:'tag',value:'one',type:'query'}, {name:'tag',value:'two',type:'query'}, {name:'off',value:'hidden',type:'query',disabled:true},
  ]}}});
  editDraft(draft,['http','params'],reorderRows(draft.value.http.params,0,1));
  assert.equal(requestUrlWithParams(draft.value.http.url,draft.value.http.params),'/users?tag=two&tag=one');
  assert.deepEqual(draft.edits[0].path,['http','params']);
  assert.equal(draft.edits[0].value[2].disabled,true);
});
