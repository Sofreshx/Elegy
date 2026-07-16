import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const repoRoot = resolve(import.meta.dirname, '../../..')
const files = execFileSync('git', ['-C', repoRoot, 'ls-files', '--cached', '--others', '--exclude-standard', '--', 'plugins/accounts'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(Boolean)
const excluded = /(^|\/)(tests?|artifacts|target|node_modules)(\/|$)|(^|\/)(Cargo|package)-lock\./
const patterns = [
  /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/,
  /AKIA[0-9A-Z]{16}/,
  /gh[pousr]_[A-Za-z0-9_]{30,}/,
  /AIza[0-9A-Za-z_-]{35}/,
  /(?:password|client_secret|refresh_token|api_key)\s*[:=]\s*["'][^"']{12,}["']/i,
]
const findings = []
for (const file of files.filter(file => !excluded.test(file.replaceAll('\\', '/')))) {
  let text
  try { text = readFileSync(resolve(repoRoot, file), 'utf8') } catch { continue }
  for (const pattern of patterns) if (pattern.test(text)) findings.push({ file, pattern: pattern.source })
}
if (findings.length) { console.error(JSON.stringify(findings, null, 2)); process.exit(1) }
console.log(`Secret scan passed for ${files.length} tracked and untracked plugin files.`)
