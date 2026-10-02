import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const appManifest = 'apps/desktop/package.json';
const tauriConfig = 'apps/desktop/src-tauri/tauri.conf.json';
const planFile = '.changeset/release-plan.json';
const cli = fileURLToPath(new URL('../node_modules/@changesets/cli/bin.js', import.meta.url));
const json = path => JSON.parse(readFileSync(path, 'utf8'));
const git = args => execFileSync('git', args, { encoding: 'utf8' }).trim();
const run = (command, args) => execFileSync(command, args, { stdio: 'inherit' });
const alpha = version => /^\d+\.\d+\.\d+-alpha\.\d+$/.test(version);

export function checkVersions() {
  const { version, private: privatePackage } = json(appManifest);
  assert.equal(privatePackage, true, 'Desktop must remain a private npm package');
  assert.equal(json(tauriConfig).version, version, 'Tauri version differs from desktop');
  const cargo = readFileSync('Cargo.toml', 'utf8');
  assert.equal(cargo.match(/\[workspace\.package\]\s*\nversion = "([^"]+)"/)?.[1], version,
    'Rust workspace version differs from desktop');
  const lock = readFileSync('Cargo.lock', 'utf8');
  for (const name of ['nimblepost-core', 'nimblepost-cli', 'nimblepost-desktop']) {
    assert.equal(lock.match(new RegExp(`name = "${name}"\\nversion = "([^"]+)"`))?.[1], version,
      `${name} lockfile version differs from desktop`);
  }
  assert.equal(json('package-lock.json').packages['apps/desktop'].version, version,
    'npm lockfile version differs from desktop');
  return version;
}

function version() {
  const previousVersion = checkVersions();
  run(process.execPath, [cli, 'version']);
  const { version } = json(appManifest);
  assert.notEqual(version, previousVersion, 'No new version was generated');
  assert(alpha(version), 'The current release channel must remain alpha');
  const cargo = readFileSync('Cargo.toml', 'utf8');
  writeFileSync('Cargo.toml', cargo.replace(/(\[workspace\.package\]\s*\nversion = ")[^"]+(")/, `$1${version}$2`));
  const tauri = readFileSync(tauriConfig, 'utf8');
  writeFileSync(tauriConfig, tauri.replace(/("version":\s*")[^"]+(")/, `$1${version}$2`));
  run('cargo', ['update', '--workspace']);
  run('npm', ['install', '--package-lock-only', '--ignore-scripts', '--no-audit', '--no-fund']);
  checkVersions();
  writeFileSync(planFile, `${JSON.stringify({ previousVersion, version }, null, 2)}\n`);
}

export function releaseBatch(base) {
  assert(/^[a-f0-9]{40}$/.test(base), 'Expected a full base commit SHA');
  if (/^0+$/.test(base)) return { pending: false };
  const files = git(['diff', '--name-only', base, 'HEAD']).split('\n').filter(Boolean);
  if (!files.includes(planFile)) return { pending: false };
  const plan = json(planFile);
  const before = JSON.parse(git(['show', `${base}:${appManifest}`]));
  assert.equal(plan.previousVersion, before.version, 'Release plan base version differs');
  assert.equal(plan.version, checkVersions(), 'Release plan target version differs');
  assert.notEqual(plan.version, plan.previousVersion, 'Release plan must advance the version');
  assert(alpha(plan.version), 'Expected an ALPHA release');
  const generated = new Set([appManifest, 'apps/desktop/CHANGELOG.md', tauriConfig,
    'Cargo.toml', 'Cargo.lock', 'package-lock.json', planFile, '.changeset/pre.json']);
  assert(files.every(path => generated.has(path) || /^\.changeset\/(?:pre\/)?[a-z0-9-]+\.md$/.test(path)),
    `Release PRs must contain only generated version changes: ${files.join(', ')}`);
  return { pending: true, version: plan.version };
}

export function releaseNotes(version) {
  assert.equal(version, checkVersions(), 'Release version differs from repository');
  assert(alpha(version), 'Expected an ALPHA release');
  const changelog = readFileSync('apps/desktop/CHANGELOG.md', 'utf8');
  const heading = `## ${version}\n`;
  const start = changelog.indexOf(heading);
  assert(start >= 0, 'Release version is missing from the changelog');
  const changes = changelog.slice(start + heading.length).split('\n## ')[0].trim();
  return `${changes}\n\n## macOS installation\n\n` +
    'Universal build for Apple Silicon and Intel. Open the DMG and drag NimblePost into Applications.\n\n' +
    'This ALPHA uses ad-hoc signing, without an Apple Developer ID certificate or notarization. ' +
    'After attempting the first launch, users who trust this download can select ' +
    'System Settings → Privacy & Security → Open Anyway.\n\n' +
    'Instructions: https://support.apple.com/en-us/102445\n\n' +
    'Unsaved drafts are not restored after restarting. Windows and Linux installers are not included.\n';
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [command, argument, output] = process.argv.slice(2);
  if (command === 'version') version();
  else if (command === 'check') { checkVersions(); console.log('Desktop, Tauri and Rust versions match.'); }
  else if (command === 'batch') {
    const batch = releaseBatch(argument);
    if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT,
      `pending=${batch.pending}\nversion=${batch.version ?? ''}\n`);
    console.log(JSON.stringify(batch));
  } else if (command === 'notes') {
    assert(output, 'Expected a notes output path');
    writeFileSync(output, releaseNotes(argument));
  } else throw new Error('Usage: node scripts/changesets.mjs <version|check|batch BASE_SHA|notes VERSION OUTPUT>');
}
