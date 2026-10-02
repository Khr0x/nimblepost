import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { parse } from 'yaml';

const root = fileURLToPath(new URL('../', import.meta.url));

test('real Changesets versioning synchronizes the app, detects release merges and prepares notes', () => {
  const directory = mkdtempSync(join(tmpdir(), 'nimblepost-changesets-'));
  const git = args => execFileSync('git', args, { cwd: directory, encoding: 'utf8' }).trim();
  const command = (...args) => execFileSync(process.execPath, ['scripts/changesets.mjs', ...args], {
    cwd: directory, encoding: 'utf8', env: { ...process.env, CARGO_NET_OFFLINE: 'true', NPM_CONFIG_OFFLINE: 'true' },
  });
  const commit = () => {
    git(['add', '.']);
    git(['-c', 'user.name=Release test', '-c', 'user.email=release@example.invalid', 'commit', '-m', 'test']);
  };
  try {
    for (const path of ['package.json', 'package-lock.json', 'Cargo.toml', 'Cargo.lock',
      'crates/core/Cargo.toml', 'bins/cli/Cargo.toml', 'apps/desktop/src-tauri/Cargo.toml',
      'apps/desktop/package.json', 'apps/desktop/src-tauri/tauri.conf.json', 'scripts/changesets.mjs']) {
      mkdirSync(dirname(join(directory, path)), { recursive: true });
      cpSync(join(root, path), join(directory, path));
    }
    // Exercise Cargo's version update without downloading application dependencies.
    const initialVersion = JSON.parse(readFileSync(join(directory, 'apps/desktop/package.json'))).version;
    const crates = [['crates/core', 'nimblepost-core'], ['bins/cli', 'nimblepost-cli'],
      ['apps/desktop/src-tauri', 'nimblepost-desktop']];
    for (const [path, name] of crates) {
      writeFileSync(join(directory, path, 'Cargo.toml'),
        `[package]\nname = "${name}"\nversion.workspace = true\nedition.workspace = true\n`);
    }
    writeFileSync(join(directory, 'Cargo.lock'), 'version = 4\n' + crates.map(([, name]) =>
      `\n[[package]]\nname = "${name}"\nversion = "${initialVersion}"\n`).join(''));
    mkdirSync(join(directory, '.changeset'));
    cpSync(join(root, '.changeset/config.json'), join(directory, '.changeset/config.json'));
    writeFileSync(join(directory, '.changeset/pre.json'), '{"mode":"pre","tag":"alpha"}\n');
    writeFileSync(join(directory, '.changeset/test-release.md'), '---\n"@nimblepost/desktop": patch\n---\n\nFix request editing.\n');
    for (const path of ['crates/core/src/lib.rs', 'bins/cli/src/main.rs',
      'apps/desktop/src-tauri/src/lib.rs', 'apps/desktop/src-tauri/src/main.rs']) {
      mkdirSync(dirname(join(directory, path)), { recursive: true });
      writeFileSync(join(directory, path), 'fn main() {}\n');
    }
    symlinkSync(join(root, 'node_modules'), join(directory, 'node_modules'), 'dir');
    writeFileSync(join(directory, '.gitignore'), 'node_modules/\n');
    git(['init', '--initial-branch=main']);
    commit();
    const base = git(['rev-parse', 'HEAD']);
    assert.equal(JSON.parse(command('batch', base)).pending, false);
    command('version');
    const plan = JSON.parse(readFileSync(join(directory, '.changeset/release-plan.json')));
    assert.equal(plan.previousVersion, initialVersion);
    assert.match(plan.version, /^\d+\.\d+\.\d+-alpha\.\d+$/);
    assert.notEqual(plan.version, initialVersion);
    assert.equal(existsSync(join(directory, '.changeset/test-release.md')), false);
    assert.equal(existsSync(join(directory, '.changeset/pre/test-release.md')), true);
    assert.match(command('check'), /versions match/);
    commit();
    assert.deepEqual(JSON.parse(command('batch', base)), { pending: true, version: plan.version });
    const notes = join(directory, 'notes.md');
    command('notes', plan.version, notes);
    assert.match(readFileSync(notes, 'utf8'), /Fix request editing/);
    assert.match(readFileSync(notes, 'utf8'), /ad-hoc signing/);
    rmSync(notes);
    assert.equal(JSON.parse(command('batch', git(['rev-parse', 'HEAD']))).pending, false);

    writeFileSync(join(directory, 'source-change.rs'), 'fn changed() {}\n');
    commit();
    const mixed = spawnSync(process.execPath, ['scripts/changesets.mjs', 'batch', base], { cwd: directory, encoding: 'utf8' });
    assert.notEqual(mixed.status, 0);
    assert.match(mixed.stderr, /only generated version changes/);

    const config = JSON.parse(readFileSync(join(directory, 'apps/desktop/src-tauri/tauri.conf.json')));
    config.version = '9.9.9';
    writeFileSync(join(directory, 'apps/desktop/src-tauri/tauri.conf.json'), JSON.stringify(config));
    const mismatch = spawnSync(process.execPath, ['scripts/changesets.mjs', 'check'], { cwd: directory, encoding: 'utf8' });
    assert.notEqual(mismatch.status, 0);
    assert.match(mismatch.stderr, /Tauri version differs/);
  } finally { rmSync(directory, { recursive: true, force: true }); }
});

test('macOS publication follows verification, supports retries and preserves published releases', () => {
  const workflow = parse(readFileSync(join(root, '.github/workflows/release-macos.yml'), 'utf8'));
  const steps = workflow.jobs.release.steps;
  const publish = steps.findIndex(step => step.name === 'Create tag and publish verified prerelease');
  assert(publish > steps.findIndex(step => step.name === 'Verify universal app signature and disk image'));
  const directory = mkdtempSync(join(tmpdir(), 'nimblepost-publish-'));
  const mock = `
    gh() {
      printf '%s\\n' "$*" >> "$CALLS"
      case "$1 $2" in
        'api '*)
          if [ "$MODE" = initial ]; then return 1; fi
          if [ "$MODE" = conflict ]; then printf 'other-commit\\n'; else printf '%s\\n' "$GITHUB_SHA"; fi ;;
        'release view')
          if [ "$MODE" = initial ]; then return 1; fi
          case "$*" in
            *targetCommitish*)
              if [ "$MODE" = draft-conflict ]; then printf 'other-commit\\n'; else printf '%s\\n' "$GITHUB_SHA"; fi
              return 0 ;;
          esac
          if [ "$MODE" = published ]; then printf 'false\\n'; else printf 'true\\n'; fi ;;
      esac
    }
  `;
  try {
    for (const mode of ['initial', 'draft', 'published', 'conflict', 'draft-conflict']) {
      const calls = join(directory, `${mode}.log`);
      const result = spawnSync('bash', ['-e', '-c', mock + steps[publish].run], {
        encoding: 'utf8', env: { ...process.env, MODE: mode, CALLS: calls,
          RELEASE_VERSION: '0.1.1-alpha.0', GITHUB_SHA: 'verified-commit',
          GITHUB_REPOSITORY: 'owner/nimblepost', RUNNER_TEMP: directory,
          GITHUB_STEP_SUMMARY: join(directory, 'summary') },
      });
      const operations = readFileSync(calls, 'utf8');
      assert.equal(result.status, mode.endsWith('conflict') ? 1 : 0, result.stderr);
      assert.equal(operations.includes('release create'), mode === 'initial');
      assert.equal(operations.includes('release upload'), mode === 'initial' || mode === 'draft');
      assert.equal(operations.includes('release edit'), mode === 'initial' || mode === 'draft');
      if (mode === 'initial') assert.match(operations, /--target verified-commit/);
      if (mode === 'initial' || mode === 'draft') assert.match(operations, /--draft=false/);
    }
  } finally { rmSync(directory, { recursive: true, force: true }); }
});
