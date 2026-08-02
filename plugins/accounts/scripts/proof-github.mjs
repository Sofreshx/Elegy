import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { randomInt, randomUUID } from 'node:crypto';

const MARKER = { schemaVersion: 'elegy-accounts-live-proof-root/v1' };

function portablePath(path) {
  return path.replaceAll('\\', '/');
}

export function createLiveGithubProofPlan({ repoRoot, proofRoot, installRoot, accountCenterPort, pipeName }) {
  const root = portablePath(repoRoot);
  const isolatedLocalAppData = portablePath(proofRoot);
  const installedRoot = portablePath(installRoot);
  const sessionRoot = dirname(isolatedLocalAppData);
  const archive = `${sessionRoot}/elegy-accounts-plugin-x86_64-pc-windows-msvc.zip`;
  const evidence = `${sessionRoot}/github-proof.json`;
  const installedBinary = `${installedRoot}/elegy-accounts/bin/elegy-accounts.exe`;

  return {
    proofRootMarker: MARKER,
    environment: {
      LOCALAPPDATA: isolatedLocalAppData,
      ELEGY_ACCOUNTS_PROOF_ROOT: isolatedLocalAppData,
      ELEGY_ACCOUNT_CENTER_PORT: String(accountCenterPort),
      ELEGY_ACCOUNTS_PIPE_NAME: pipeName,
    },
    cleanupPaths: [
      `${isolatedLocalAppData}/Elegy/Accounts`,
      `${isolatedLocalAppData}/vault-backup.sqlite`,
    ],
    commands: [
      { command: 'cargo', args: ['build', '--release', '-p', 'elegy-accounts'], cwd: root },
      {
        command: 'cargo',
        args: [
          'run', '-q', '-p', 'elegy-tooling', '--bin', 'elegy-plugin-packaging', '--',
          'pack', '--plugin', 'plugins/accounts', '--output', archive, '--binary',
          'target/release/elegy-accounts.exe', '--binary-name', 'bin/elegy-accounts.exe',
        ],
        cwd: root,
      },
      {
        command: 'cargo',
        args: [
          'run', '-q', '-p', 'elegy-tooling', '--bin', 'elegy-plugin-packaging', '--',
          'install', '--archive', archive, '--install-root', installedRoot,
        ],
        cwd: root,
      },
      {
        command: installedBinary,
        args: ['proof-github', evidence, '--consent=github-device-read-only'],
        cwd: root,
      },
    ],
  };
}

function requireLiveProofConsent() {
  if (process.env.ELEGY_LIVE_PROOF_CONSENT !== 'github-device-read-only') {
    throw new Error(
      'Refusing live GitHub proof. Set ELEGY_LIVE_PROOF_CONSENT=github-device-read-only only after approving the supervised read-only run.',
    );
  }
  if (!process.env.ELEGY_GITHUB_CLIENT_ID?.trim()) {
    throw new Error('ELEGY_GITHUB_CLIENT_ID must name a dedicated GitHub Device Flow OAuth application.');
  }
}

function run(command, { args, cwd }, environment) {
  const result = spawnSync(command, args, { cwd, env: environment, stdio: 'inherit' });
  if (result.error) throw new Error(`Unable to start ${command}: ${result.error.message}`);
  if (result.status !== 0) throw new Error(`${command} failed with exit code ${result.status ?? 1}`);
}

export function runLiveGithubProof() {
  requireLiveProofConsent();
  const repoRoot = resolve(import.meta.dirname, '../../..');
  const sessionRoot = mkdtempSync(join(tmpdir(), 'elegy-accounts-live-proof-'));
  const proofRoot = join(sessionRoot, 'localappdata');
  const installRoot = join(sessionRoot, 'installed');
  const plan = createLiveGithubProofPlan({
    repoRoot,
    proofRoot,
    installRoot,
    accountCenterPort: randomInt(49152, 65536),
    pipeName: `\\\\.\\pipe\\elegy-accounts-live-proof-${randomUUID()}`,
  });
  mkdirSync(proofRoot, { recursive: true });
  writeFileSync(join(proofRoot, '.elegy-accounts-live-proof.json'), `${JSON.stringify(plan.proofRootMarker)}\n`);

  const environment = { ...process.env, ...plan.environment };
  delete environment.ELEGY_ACCOUNTS_PROVIDER_DIR;
  delete environment.ELEGY_ACCOUNTS_TRUST_LOCAL_PACKS;
  delete environment.ELEGY_ACCOUNT_CENTER_DIST;
  try {
    for (const command of plan.commands) run(command.command, command, environment);
  } finally {
    for (const path of plan.cleanupPaths) rmSync(path, { recursive: true, force: true });
  }
  console.log(`GitHub live proof passed. Evidence: ${plan.commands.at(-1).args[1]}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    runLiveGithubProof();
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
