import { spawnSync } from 'node:child_process';

const consent = process.env.ELEGY_LIVE_PROOF_CONSENT;
if (consent !== 'github-read-only') {
  console.error(
    'Refusing live GitHub proof. Set ELEGY_LIVE_PROOF_CONSENT=github-read-only only after approving the supervised read-only run.',
  );
  process.exit(1);
}

const result = spawnSync(
  'cargo',
  [
    'run',
    '-p',
    'elegy-accounts',
    '--',
    'proof-github',
    'artifacts/live/github-proof.json',
    '--consent=github-read-only',
  ],
  { stdio: 'inherit' },
);

if (result.error) {
  console.error(`Unable to start cargo: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 1);
