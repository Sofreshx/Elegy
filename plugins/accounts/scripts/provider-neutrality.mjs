import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..')
const genericSurfaces = [
  'crates/elegy-accountd/src/provider.rs',
  'crates/elegy-accountd/src/adapter.rs',
  'crates/elegy-accountd/src/proxy.rs',
  'apps/account-center/src/App.tsx',
  'apps/account-center/src/api.ts',
  'apps/account-center/src/components/Dialogs.tsx',
  'extensions/brave/src/background.ts',
  'extensions/brave/src/popup.ts',
  'extensions/brave/src/security.ts',
]
const proofProviders = ['github', 'cloudflare', 'google']
const violations = []
for (const relative of genericSurfaces) {
  const source = readFileSync(resolve(root, relative), 'utf8').toLowerCase()
  for (const provider of proofProviders) {
    if (source.includes(provider)) violations.push(`${relative}: ${provider}`)
  }
}
if (violations.length) {
  console.error(`Compiled provider knowledge found:\n${violations.join('\n')}`)
  process.exit(1)
}
console.log(`Provider neutrality: PASS (${genericSurfaces.length} generic surfaces)`)
