import { spawnSync } from 'node:child_process'
import { mkdirSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

const pluginRoot = resolve(import.meta.dirname, '..')
const repoRoot = resolve(import.meta.dirname, '../../..')
const artifactDir = resolve(pluginRoot, 'artifacts', 'acceptance')
mkdirSync(artifactDir, { recursive: true })
mkdirSync(resolve(pluginRoot, 'artifacts', 'distribution'), { recursive: true })
mkdirSync(resolve(pluginRoot, 'artifacts', 'codex'), { recursive: true })

const checks = []
function run(id, command, args, cwd = repoRoot) {
  const started = new Date().toISOString()
  const result = spawnSync(command, args, { cwd, encoding: 'utf8', shell: process.platform === 'win32', maxBuffer: 16 * 1024 * 1024 })
  const record = { id, passed: result.status === 0, command: [command, ...args].join(' '), started, stdout: result.stdout?.slice(-12000) ?? '', stderr: result.stderr?.slice(-12000) ?? '' }
  checks.push(record)
  writeFileSync(resolve(artifactDir, `${id}.json`), JSON.stringify(record, null, 2))
  if (!record.passed) throw new Error(`${id} failed; see artifacts/acceptance/${id}.json`)
}

try {
  run('rust-format', 'cargo', ['fmt', '--all', '--', '--check'])
  run('rust-tests', 'cargo', ['test', '-p', 'elegy-accountd', '-p', 'elegy-accounts'])
  run('workspace-checks', 'npm', ['run', 'check'], pluginRoot)
  run('provider-neutrality', 'node', ['scripts/provider-neutrality.mjs'], pluginRoot)
  run('ui-e2e', 'npm', ['run', 'test:e2e'], pluginRoot)
  run('rust-vulnerability-audit', 'cargo', ['audit', '-q'])
  run('rust-policy-audit', 'cargo', ['deny', 'check', 'advisories', 'licenses', 'bans', 'sources'])
  run('npm-vulnerability-audit', 'npm', ['audit', '--audit-level=high'], pluginRoot)
  run('secret-scan', 'node', ['scripts/secret-scan.mjs'], pluginRoot)
  run('plugin-contract', 'cargo', ['run', '-q', '-p', 'elegy-tooling', '--bin', 'elegy-plugin-packaging', '--', 'verify', '--plugin', 'plugins/accounts'])
  run('skill-validation', 'cargo', ['run', '-q', '-p', 'elegy-tooling', '--bin', 'elegy-plugin-packaging', '--', 'verify', '--plugin', 'plugins/accounts'])
  run('release-build', 'cargo', ['build', '--release', '-p', 'elegy-accounts'])
  run('release-pack', 'cargo', ['run', '-q', '-p', 'elegy-tooling', '--bin', 'elegy-plugin-packaging', '--', 'pack', '--plugin', 'plugins/accounts', '--output', 'plugins/accounts/artifacts/distribution/elegy-accounts-plugin-x86_64-pc-windows-msvc.zip', '--binary', 'target/release/elegy-accounts.exe', '--binary-name', 'bin/elegy-accounts.exe'])
  run('codex-export', 'cargo', ['run', '-q', '-p', 'elegy-tooling', '--bin', 'elegy-plugin-packaging', '--', 'export', '--plugin', 'plugins/accounts', '--host', 'codex', '--output', 'plugins/accounts/artifacts/codex/elegy-accounts', '--overwrite', '--binary', 'target/release/elegy-accounts.exe', '--binary-name', 'bin/elegy-accounts.exe'])
  run('plugin-validation', 'cargo', ['run', '-q', '-p', 'elegy-tooling', '--bin', 'elegy-plugin-packaging', '--', 'verify', '--plugin', 'plugins/accounts'])
  run('packaging-smoke', 'powershell', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', 'scripts/packaging-smoke.ps1'], pluginRoot)

  const evidence = {
    'AC-01': ['ui-e2e', 'workspace-checks'], 'AC-02': ['ui-e2e', 'workspace-checks', 'packaging-smoke'],
    'AC-03': ['ui-e2e', 'rust-tests'], 'AC-04': ['rust-tests'], 'AC-05': ['rust-tests'], 'AC-06': ['rust-tests', 'workspace-checks'],
    'AC-07': ['rust-tests', 'ui-e2e'], 'AC-08': ['rust-tests'], 'AC-09': ['rust-tests'], 'AC-10': ['rust-tests'],
    'AC-11': ['rust-tests'], 'AC-12': ['rust-tests'], 'AC-13': ['rust-tests'], 'AC-14': ['rust-tests'],
    'AC-15': ['rust-tests', 'workspace-checks', 'ui-e2e'], 'AC-16': ['rust-tests'], 'AC-17': ['rust-tests'], 'AC-18': ['rust-tests'],
    'AC-19': ['workspace-checks', 'packaging-smoke'], 'AC-20': ['rust-tests', 'packaging-smoke'],
    'AC-21': ['rust-vulnerability-audit', 'rust-policy-audit', 'npm-vulnerability-audit', 'secret-scan'],
    'AC-22': ['ui-e2e'], 'AC-23': ['ui-e2e', 'workspace-checks'], 'AC-24': ['ui-e2e'],
    'AC-25': ['plugin-contract', 'plugin-validation', 'skill-validation', 'rust-tests'], 'AC-26': ['release-pack', 'codex-export', 'packaging-smoke'],
  }
  const byId = Object.fromEntries(checks.map(check => [check.id, check]))
  const criteria = Object.fromEntries(Object.entries(evidence).map(([id, ids]) => [id, { passed: ids.every(check => byId[check]?.passed), evidence: ids }]))
  const report = { generatedAt: new Date().toISOString(), passed: Object.values(criteria).every(item => item.passed), criteria, checks: checks.map(({ stdout, stderr, ...check }) => check) }
  writeFileSync(resolve(artifactDir, 'report.json'), JSON.stringify(report, null, 2))
  console.log(`Acceptance: ${report.passed ? 'PASS' : 'FAIL'} (${Object.keys(criteria).length} criteria)`)
} catch (error) {
  const report = { generatedAt: new Date().toISOString(), passed: false, error: error.message, checks: checks.map(({ stdout, stderr, ...check }) => check) }
  writeFileSync(resolve(artifactDir, 'report.json'), JSON.stringify(report, null, 2))
  console.error(error.message)
  process.exit(1)
}
