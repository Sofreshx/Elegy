import assert from 'node:assert/strict';
import test from 'node:test';
import { createLiveGithubProofPlan } from './proof-github.mjs';

test('plans the GitHub proof through a freshly packaged and installed binary', () => {
  const plan = createLiveGithubProofPlan({
    repoRoot: 'C:/work/elegy',
    proofRoot: 'C:/proof/localappdata',
    installRoot: 'C:/proof/installed',
    accountCenterPort: 54321,
    pipeName: String.raw`\\.\pipe\elegy-accounts-live-proof-test`,
  });

  assert.deepEqual(plan.proofRootMarker, {
    schemaVersion: 'elegy-accounts-live-proof-root/v1',
  });
  assert.equal(plan.environment.LOCALAPPDATA, 'C:/proof/localappdata');
  assert.equal(plan.environment.ELEGY_ACCOUNTS_PROOF_ROOT, 'C:/proof/localappdata');
  assert.equal(plan.environment.ELEGY_ACCOUNT_CENTER_PORT, '54321');
  assert.equal(plan.environment.ELEGY_ACCOUNTS_PIPE_NAME, String.raw`\\.\pipe\elegy-accounts-live-proof-test`);
  assert.deepEqual(plan.cleanupPaths, [
    'C:/proof/localappdata/Elegy/Accounts',
    'C:/proof/localappdata/vault-backup.sqlite',
  ]);

  assert.deepEqual(plan.commands[0], {
    command: 'cargo',
    args: ['build', '--release', '-p', 'elegy-accounts'],
    cwd: 'C:/work/elegy',
  });
  assert.deepEqual(plan.commands[1], {
    command: 'cargo',
    args: [
      'run', '-q', '-p', 'elegy-tooling', '--bin', 'elegy-plugin-packaging', '--',
      'pack', '--plugin', 'plugins/accounts', '--output',
      'C:/proof/elegy-accounts-plugin-x86_64-pc-windows-msvc.zip', '--binary',
      'target/release/elegy-accounts.exe', '--binary-name', 'bin/elegy-accounts.exe',
    ],
    cwd: 'C:/work/elegy',
  });
  assert.deepEqual(plan.commands[2], {
    command: 'cargo',
    args: [
      'run', '-q', '-p', 'elegy-tooling', '--bin', 'elegy-plugin-packaging', '--',
      'install', '--archive', 'C:/proof/elegy-accounts-plugin-x86_64-pc-windows-msvc.zip',
      '--install-root', 'C:/proof/installed',
    ],
    cwd: 'C:/work/elegy',
  });
  assert.deepEqual(plan.commands[3], {
    command: 'C:/proof/installed/elegy-accounts/bin/elegy-accounts.exe',
    args: ['proof-github', 'C:/proof/github-proof.json', '--consent=github-device-read-only'],
    cwd: 'C:/work/elegy',
  });
});

test('never plans GitHub CLI or a source cargo proof invocation', () => {
  const plan = createLiveGithubProofPlan({
    repoRoot: 'C:/work/elegy',
    proofRoot: 'C:/proof/localappdata',
    installRoot: 'C:/proof/installed',
    accountCenterPort: 54321,
    pipeName: String.raw`\\.\pipe\elegy-accounts-live-proof-test`,
  });
  const rendered = plan.commands.map(({ command, args }) => [command, ...args].join(' '));

  assert.equal(rendered.some(command => /(^|\s)gh(\s|$)/.test(command)), false);
  assert.equal(
    rendered.some(command => command.includes('cargo run -p elegy-accounts') || command.includes('cargo run -q -p elegy-accounts')),
    false,
  );
  assert.equal(rendered.at(-1).startsWith('C:/proof/installed/elegy-accounts/bin/elegy-accounts.exe proof-github'), true);
});
