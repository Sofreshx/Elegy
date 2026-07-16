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

export const seedAccounts: Account[] = [
  {
    id: 'cloudflare-alex', provider: 'Cloudflare', mark: 'CF', markColor: '#a84308',
    identity: 'token:verified-demo', health: 'Healthy', connection: 'Connected with a scoped token',
    connectedAt: 'Last connected today at 10:24 AM',
    grants: [
      { id: 'cf-codex', client: 'Codex', summary: 'Can read zones, DNS records, and settings.', limitation: 'No write access.' },
      { id: 'cf-browser', client: 'Browser', summary: 'Can read account profile and zones.', limitation: 'No write access.' },
    ],
  },
  {
    id: 'github-alex', provider: 'GitHub', mark: 'GH', markColor: '#111827',
    identity: 'alex@acme.dev', health: 'Healthy', connection: 'Connected with GitHub device flow',
    connectedAt: 'Last verified yesterday',
    grants: [{ id: 'gh-codex', client: 'Codex', summary: 'Can read repositories and prepare pull requests.', limitation: 'Publishing requires confirmation.' }],
  },
]
