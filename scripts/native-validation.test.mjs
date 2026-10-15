import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { parse } from 'yaml';

const workflow = parse(readFileSync(new URL('../.github/workflows/native-validation.yml', import.meta.url), 'utf8'));
const lifecycle = workflow.jobs.native.steps.find(step => step.name === 'Install, launch and uninstall production Debian package').run;

test('Debian lifecycle rejects installation, launch, uninstall and data-preservation failures', () => {
  const directory = mkdtempSync(join(tmpdir(), 'nimblepost native '));
  const mocks = `
    dpkg-deb() {
      if [ "$3" = Architecture ]; then
        if [ "$MODE" = architecture ]; then printf 'arm64\\n'; else printf 'amd64\\n'; fi
      else printf 'nimblepost\\n'; fi
    }
    sudo() {
      if [ "$2" = install ] && [ "$MODE" = install ]; then return 1; fi
      if [ "$2" = remove ]; then
        printf 'remove\\n' >> "$RUNNER_TEMP/operations"
        if [ "$MODE" = uninstall ]; then return 1; fi
        if [ "$MODE" = data ]; then
          printf 'corrupt\\n' > "$RUNNER_TEMP/nimblepost-external-collection/opencollection.yml"
        fi
      fi
    }
    dbus-run-session() { [ "$MODE" != launch ]; }
    test() {
      if [ "$*" = '! -e /usr/bin/NimblePost' ]; then [ "$MODE" != binary ]; else builtin test "$@"; fi
    }
  `;
  try {
    const bin = join(directory, 'bin');
    mkdirSync(bin);
    // find -exec needs an executable, rather than a shell function, on macOS too.
    writeFileSync(join(bin, 'sha256sum'), `#!/usr/bin/env node
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
for (const path of process.argv.slice(2)) console.log(createHash('sha256').update(readFileSync(path)).digest('hex') + '  ' + path);
`, { mode: 0o755 });
    // Extensionless Node entry points use the nearest package type.
    writeFileSync(join(directory, 'package.json'), '{"type":"module"}');
    for (const mode of ['ok', 'architecture', 'packages', 'install', 'launch', 'uninstall', 'binary', 'data']) {
      const cwd = join(directory, mode);
      const runner = join(cwd, 'runner temp');
      mkdirSync(join(cwd, 'examples/basic-http'), { recursive: true });
      mkdirSync(join(cwd, 'target/release/bundle/deb'), { recursive: true });
      mkdirSync(join(runner, 'native-validation'), { recursive: true });
      writeFileSync(join(cwd, 'examples/basic-http/opencollection.yml'), 'original\n');
      writeFileSync(join(cwd, 'target/release/bundle/deb/app.deb'), 'fixture');
      if (mode === 'packages') writeFileSync(join(cwd, 'target/release/bundle/deb/other.deb'), 'fixture');
      const result = spawnSync('bash', ['-e', '-o', 'pipefail', '-c', mocks + lifecycle], {
        cwd, encoding: 'utf8', env: { ...process.env, MODE: mode, RUNNER_TEMP: runner, PATH: `${bin}:${process.env.PATH}` },
      });
      assert.equal(result.status === 0, mode === 'ok', `${mode}: ${result.stderr}`);
      if (mode === 'ok') {
        assert.match(readFileSync(join(runner, 'native-validation/linux-lifecycle.log'), 'utf8'), /PASS: Debian/);
        assert.equal(readFileSync(join(runner, 'operations'), 'utf8'), 'remove\n');
        assert.equal(readFileSync(join(runner, 'nimblepost-external-collection/opencollection.yml'), 'utf8'), 'original\n');
      }
    }
  } finally { rmSync(directory, { recursive: true, force: true }); }
});
