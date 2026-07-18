export type Grant = {
  id: string
  client: 'Codex' | 'Browser' | 'Holon'
  summary: string
  limitation: string
}

export type Account = {
  id: string
  provider: string
  mark: string
  markColor: string
  identity: string
  health: 'Healthy' | 'Limited'
  connection: string
  connectedAt: string
  grants: Grant[]
}

export const seedProviders = [
  {
    schema_version: 'elegy-account-provider/v1' as const, id: 'example-edge', display_name: 'Example Edge', version: '1.0.0', publisher: 'demo',
    browser_origins: ['https://edge.example.test'], auth_profiles: [{ id: 'token', method: 'api_token' as const, scopes: ['dns.read'], creation_url: 'https://edge.example.test/tokens' }], operations: { 'dns.read': ['dns.read'] },
  },
  {
    schema_version: 'elegy-account-provider/v1' as const, id: 'code-forge', display_name: 'Code Forge', version: '1.0.0', publisher: 'demo',
    browser_origins: ['https://code.example.test'], auth_profiles: [{ id: 'device', method: 'device_authorization' as const, scopes: ['profile.read'] }], operations: { 'profile.read': ['profile.read'] },
  },
]

export const seedAccounts: Account[] = [
  {
    id: 'edge-alex', provider: 'Example Edge', mark: 'EE', markColor: '#a84308',
    identity: 'token:verified-demo', health: 'Healthy', connection: 'Connected with a scoped token',
    connectedAt: 'Last connected today at 10:24 AM',
    grants: [
      { id: 'edge-codex', client: 'Codex', summary: 'Can read zones, DNS records, and settings.', limitation: 'No write access.' },
      { id: 'edge-browser', client: 'Browser', summary: 'Can read account profile and zones.', limitation: 'No write access.' },
    ],
  },
  {
    id: 'forge-alex', provider: 'Code Forge', mark: 'CF', markColor: '#111827',
    identity: 'alex@acme.dev', health: 'Healthy', connection: 'Connected with device authorization',
    connectedAt: 'Last verified yesterday',
    grants: [{ id: 'forge-codex', client: 'Codex', summary: 'Can read repositories and prepare pull requests.', limitation: 'Publishing requires confirmation.' }],
  },
]
