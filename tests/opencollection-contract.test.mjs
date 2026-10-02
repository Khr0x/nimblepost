import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import Ajv from 'ajv';
import { parseDocument } from 'yaml';

const root = new URL('../', import.meta.url);
const fixtureRoot = new URL('tests/fixtures/opencollection/', root);
const manifest = JSON.parse(readFileSync(new URL('manifest.json', fixtureRoot)));
const schemaBytes = readFileSync(new URL(manifest.schema.path, root));
const schema = JSON.parse(schemaBytes);
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const ajv = new Ajv({ strict: false, allErrors: true });
ajv.addSchema(schema);

function readYaml(url) {
  const source = readFileSync(url, 'utf8');
  const doc = parseDocument(source, { version: '1.2', uniqueKeys: true });
  assert.deepEqual(doc.errors, [], `YAML inválido: ${fileURLToPath(url)}`);
  assert.deepEqual(doc.warnings, [], `YAML no soportado: ${fileURLToPath(url)}`);
  return doc.toJS({ maxAliasCount: 0 });
}

function validator(definition) {
  const id = definition ? `${schema.$id}#/$defs/${definition}` : schema.$id;
  const validate = ajv.getSchema(id);
  assert.ok(validate, `Definición no encontrada: ${id}`);
  return validate;
}

test('el esquema está fijado por versión, revisión y hash', () => {
  assert.equal(schema.$id, 'https://schema.opencollection.com/opencollection/v1.0.0.json');
  assert.equal(manifest.schema.formatVersion, '1.0.0');
  assert.equal(manifest.schema.packageVersion, '0.14.0');
  assert.match(manifest.schema.commit, /^[a-f0-9]{40}$/);
  assert.equal(hash(schemaBytes), manifest.schema.sha256);
});

for (const fixture of manifest.fixtures) {
  test(`fixture: ${fixture.path}`, () => {
    const url = new URL(fixture.path, fixtureRoot);
    assert.equal(hash(readFileSync(url)), fixture.sha256);
    assert.match(fixture.commit, /^[a-f0-9]{40}$/);
    assert.ok(fixture.sourceUrl.includes(fixture.commit));
    const validate = validator(fixture.definition);
    assert.equal(validate(readYaml(url)), fixture.schemaValid,
      JSON.stringify(validate.errors, null, 2));
    for (const issue of fixture.expectedIssues ?? []) {
      assert.ok(validate.errors?.some(error =>
        error.instancePath === issue.instancePath &&
        error.keyword === issue.keyword &&
        (!issue.property || error.params.additionalProperty === issue.property)),
      `No se detectó ${JSON.stringify(issue)}`);
    }
  });
}

for (const entry of manifest.examples) {
  test(`ejemplo propio: ${entry.path}`, () => {
    const data = readYaml(new URL(entry.path, root));
    const validate = validator(entry.definition);
    assert.equal(validate(data), true, JSON.stringify(validate.errors, null, 2));
    if (entry.definition === null) {
      assert.equal(data.opencollection, '1.0.0');
      assert.ok(data.info.name);
      assert.equal(data.bundled, false);
    } else if (entry.definition === 'HttpRequest') {
      assert.equal(data.info.type, 'http');
      assert.ok(data.http.method);
      assert.ok(data.http.url);
    }
  });
}

test('todos los ejemplos YAML están incluidos en el manifest', () => {
  const listed = manifest.examples.map(entry => entry.path).sort();
  const actual = readdirSync(new URL('examples/basic-http/', root), {
    recursive: true,
  }).filter(path => /\.ya?ml$/.test(path))
    .map(path => `examples/basic-http/${path}`).sort();
  assert.deepEqual(actual, listed);
});

test('los ejemplos mantienen plantillas y declaración de secreto sin valor', () => {
  const env = readYaml(new URL('examples/basic-http/environments/local.yml', root));
  const token = env.variables.find(variable => variable.name === 'token');
  assert.equal(token.secret, true);
  assert.equal(Object.hasOwn(token, 'value'), false);
  const request = readYaml(new URL('examples/basic-http/users/create.yml', root));
  assert.equal(request.http.auth.token, '{{token}}');
});
