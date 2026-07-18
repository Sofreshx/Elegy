export type DiscoveryHint = { providerId: string; origin: string; verified: false }
export type ProviderDescriptor = { id: string; displayName: string; browserOrigins: string[] }

const secretKeys = new Set([
  'authorization', 'password', 'cookie', 'set-cookie', 'access_token',
  'refresh_token', 'api_key', 'client_secret', 'secret', 'token',
])

export function discoveryHintForUrl(rawUrl: string, providers: ProviderDescriptor[]): DiscoveryHint | null {
  let url: URL
  try { url = new URL(rawUrl) } catch { return null }
  const providerByOrigin = new Map(providers.flatMap(provider => provider.browserOrigins.map(origin => [origin, provider.id] as const)))
  const providerId = providerByOrigin.get(url.origin)
  return providerId ? { providerId, origin: url.origin, verified: false } : null
}

export function sanitizeNativeMessage<T>(message: T): T {
  assertNoSecretFields(message, '$')
  return structuredClone(message)
}

function assertNoSecretFields(value: unknown, path: string): void {
  if (!value || typeof value !== 'object') return
  for (const [key, child] of Object.entries(value)) {
    if (secretKeys.has(key.toLowerCase())) throw new Error(`Secret-bearing field rejected at ${path}.${key}`)
    assertNoSecretFields(child, `${path}.${key}`)
  }
}
