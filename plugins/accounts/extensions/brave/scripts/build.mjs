import { build } from 'esbuild'
import { copyFile, mkdir } from 'node:fs/promises'
import { resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..')
const outdir = resolve(root, 'dist')
await mkdir(outdir, { recursive: true })
await build({
  entryPoints: [resolve(root, 'src/background.ts'), resolve(root, 'src/popup.ts')],
  bundle: true, format: 'esm', target: 'chrome120', outdir, sourcemap: false,
})
await Promise.all([
  copyFile(resolve(root, 'manifest.json'), resolve(outdir, 'manifest.json')),
  copyFile(resolve(root, 'popup.html'), resolve(outdir, 'popup.html')),
])
