import { cpSync, rmSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..')
for (const [source, target] of [
  ['apps/account-center/dist', 'ui/account-center'],
  ['extensions/brave/dist', 'browser/brave'],
]) {
  const sourcePath = resolve(root, source)
  const targetPath = resolve(root, target)
  rmSync(targetPath, { recursive: true, force: true })
  cpSync(sourcePath, targetPath, { recursive: true })
}

console.log('Synced packaged Account Center and Brave assets.')
