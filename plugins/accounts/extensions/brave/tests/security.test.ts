import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { discoveryHintForUrl, sanitizeNativeMessage } from '../src/security'

describe('Brave bridge security boundary', () => {
  it('uses optional provider hosts and never requests password or cookie APIs', () => {
    const manifest = JSON.parse(readFileSync(resolve(import.meta.dirname, '../manifest.json'), 'utf8'))
    expect(manifest.manifest_version).toBe(3)
    expect(manifest.permissions).toContain('nativeMessaging')
    expect(manifest.permissions).not.toContain('cookies')
    expect(manifest.permissions).not.toContain('passwordsPrivate')
    expect(manifest.host_permissions ?? []).toEqual([])
    expect(manifest.optional_host_permissions ?? []).toEqual([])
    expect(manifest.optional_host_permissions).not.toContain('<all_urls>')
  })

  it('discovers only allowlisted provider origins and marks hints unverified', () => {
    const providers = [{ id: 'synthetic-edge', displayName: 'Synthetic Edge', browserOrigins: ['https://edge.example.test'] }]
    expect(discoveryHintForUrl('https://edge.example.test/profile/tokens', providers)).toEqual({
      providerId: 'synthetic-edge',
      origin: 'https://edge.example.test',
      verified: false,
    })
    expect(discoveryHintForUrl('https://malicious.example/synthetic-edge', providers)).toBeNull()
  })

  it('rejects secret-bearing fields before native messaging', () => {
    expect(() => sanitizeNativeMessage({ type: 'discovery', cookie: 'secret' })).toThrow(/secret-bearing/i)
    expect(() => sanitizeNativeMessage({ type: 'discovery', password: 'secret' })).toThrow(/secret-bearing/i)
    expect(sanitizeNativeMessage({ type: 'discovery', providerId: 'github' })).toEqual({ type: 'discovery', providerId: 'github' })
  })
})
