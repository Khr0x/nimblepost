import assert from 'node:assert/strict';
import { test } from 'node:test';
import { draftFrom } from '../../src/lib/features/requests/draft.ts';
import { editRequestUrl, requestUrlWithParams } from '../../src/lib/features/requests/request-url.ts';

test('active query params appear in the URL, before the fragment, without losing existing query text', () => {
  const rows = [
    { type: 'query', name: 'tag', value: 'a b&c' },
    { type: 'query', name: 'tag', value: '{{tag}}' },
    { type: 'query', name: 'ignore', value: '{{missing}}', disabled: true },
    { type: 'path', name: 'id', value: '42' },
  ];
  assert.equal(requestUrlWithParams('{{baseUrl}}/users?existing=1#results', rows), '{{baseUrl}}/users?existing=1&tag=a+b%26c&tag={{tag}}#results');
  rows[0].disabled = true;
  assert.equal(requestUrlWithParams('/users', rows), '/users?tag={{tag}}');
  assert.equal(requestUrlWithParams('/users', []), '/users');
});

test('query variable names and typed string values remain editable placeholders', () => {
  assert.equal(requestUrlWithParams('/users', [{type:'query', name:'{{ key }}', value:{type:'string', data:'prefix {{value}} /'}}]), '/users?{{ key }}=prefix+{{value}}+%2F');
});

test('editing the displayed URL updates params once and preserves disabled rows and metadata', () => {
  const draft = draftFrom({revision:1, diagnostics:[], document:{http:{url:'/users?existing=1#results', params:[
    {type:'query', name:'tag', value:'one', description:'Keep me'},
    {type:'query', name:'ignore', value:'old', disabled:true},
    {type:'query', name:'tag', value:{type:'string', data:'two'}, description:'Second'},
  ]}}});
  const displayed = requestUrlWithParams(draft.value.http.url, draft.value.http.params);
  editRequestUrl(draft, displayed.replace('tag=one', 'tag=edited'));
  assert.equal(draft.value.http.url, '/users#results');
  assert.equal(requestUrlWithParams(draft.value.http.url, draft.value.http.params), displayed.replace('tag=one', 'tag=edited'));
  assert.equal(draft.value.http.params.filter(row => !row.disabled && row.name === 'tag').length, 2);
  assert.equal(draft.value.http.params.find(row => row.value === 'edited').description, 'Keep me');
  assert.deepEqual(draft.value.http.params.find(row => row.disabled), {type:'query', name:'ignore', value:'old', disabled:true});
  assert.equal(draft.value.http.params.at(-1).value.data, 'two');
});

test('deleting query text removes active params and keeps disabled and unsupported rows', () => {
  const draft = draftFrom({revision:1, diagnostics:[], document:{http:{url:'/users', params:[
    {type:'query', name:'tag', value:'one'}, {type:'query', name:'off', value:'two', disabled:true}, {type:'path',name:'id',value:'3'},
  ]}}});
  editRequestUrl(draft, '/users#results');
  assert.deepEqual(draft.value.http.params.map(row => row.name), ['off', 'id']);
  assert.equal(requestUrlWithParams(draft.value.http.url, draft.value.http.params), '/users#results');
});
