import type { Account, Grant } from './types'

export type BrokerRequest = {
  id: string
  kind: 'access' | 'creation'
  status: string
  account_id?: string
  provider?: string
  client_id?: string
  purpose: string
  operations: string[]
  duration_minutes: number
}

export type BrokerGrant = {
  id: string
  account_id: string
  provider: string
  client_id: string
  purpose: string
  operations: string[]
  expires_at: string
  revoked: boolean
}

export type AuditEvent = { time: string; event: string; account_id?: string; detail: Record<string, unknown> }
export type AuthorizationSession = {
  id: string; provider: string; status: 'waiting_for_user' | 'interaction_required' | 'connected' | 'superseded' | 'cancelled'
  user_code: string; verification_uri: string; expires_at: string; interval_seconds: number; next_poll_at: string
  attempts: number; last_error?: string; created_at: string; updated_at: string
}

export type AuthProfile = {
  id: string
  method: 'oauth_pkce' | 'device_authorization' | 'api_token' | 'http_basic' | 'client_credentials' | 'service_credential'
  scopes: string[]
  creation_url?: string
  credential_fields?: CredentialField[]
}
export type CredentialField = { id: string; label: string; secret: boolean; autocomplete?: string }
export type ProviderPack = {
  schema_version: 'elegy-account-provider/v1'
  id: string
  display_name: string
  version: string
  publisher: string
  browser_origins: string[]
  auth_profiles: AuthProfile[]
  operations: Record<string, string[]>
}

type AccountMetadata = { id: string; provider: string; verified_identity: string; auth_method: string; created_at: string }
export type AccountState = { accounts: Account[]; requests: BrokerRequest[]; audit: AuditEvent[]; authorizations: AuthorizationSession[]; providers: ProviderPack[] }
export type ConnectionStart =
  | { mode: 'browser'; authorization_url: string }
  | { mode: 'device'; provider: string; request_id: string; user_code: string; verification_uri: string; expires_in: number; interval: number }
  | { mode: 'manual_credential'; provider: string; profile: string; method: AuthProfile['method']; creation_url?: string; credential_fields: CredentialField[] }
export type DeviceStatus = { status: 'pending' | 'connected' | 'denied' | 'expired' | 'failed'; retry_after?: number; message?: string }

export async function loadState(signal?: AbortSignal): Promise<AccountState> {
  const response = await fetch('/api/state', { signal, credentials: 'same-origin' })
  if (!response.ok) throw new Error('The local account broker is unavailable.')
  const data = await response.json() as { accounts: AccountMetadata[]; requests: BrokerRequest[]; grants: BrokerGrant[]; audit: AuditEvent[]; authorizations?: AuthorizationSession[]; providers?: ProviderPack[] }
  const providers = data.providers ?? []
  const providerById = new Map(providers.map(provider => [provider.id, provider]))
  return {
    accounts: data.accounts.map(account => toAccount(account, data.grants.filter(grant => grant.account_id === account.id && !grant.revoked), providerById.get(account.provider))),
    requests: data.requests,
    audit: data.audit,
    authorizations: data.authorizations ?? [],
    providers,
  }
}

export async function approveRequest(id: string) { return mutate(`/api/requests/${encodeURIComponent(id)}/approve`) }
export async function cancelRequest(id: string) { return mutate(`/api/requests/${encodeURIComponent(id)}/cancel`) }
export async function disconnectAccount(id: string) { return mutate(`/api/accounts/${encodeURIComponent(id)}/disconnect`) }
export async function revokeGrant(id: string) { return mutate(`/api/grants/${encodeURIComponent(id)}/revoke`) }
export async function startConnection(provider: string): Promise<ConnectionStart> {
  const response = await fetch(`/api/connections/${encodeURIComponent(provider)}/start`, { method: 'POST', credentials: 'same-origin', headers: { 'X-Elegy-Intent': 'user-action' } })
  const body = await response.json() as { mode?: string; provider?: string; profile?: string; method?: AuthProfile['method']; authorization_url?: string; creation_url?: string; credential_fields?: CredentialField[]; request_id?: string; user_code?: string; verification_uri?: string; expires_in?: number; interval?: number; message?: string }
  if (!response.ok) throw new Error(body.message ?? 'This provider connection is not configured yet.')
  if (body.mode === 'device' && body.request_id && body.user_code && body.verification_uri) return { ...body, provider } as Extract<ConnectionStart, { mode: 'device' }>
  if (body.mode === 'manual_credential' && body.profile && body.method && body.credential_fields) return { ...body, provider } as Extract<ConnectionStart, { mode: 'manual_credential' }>
  if (body.authorization_url) return { mode: 'browser', authorization_url: body.authorization_url }
  throw new Error('The provider returned an invalid authorization response.')
}

export async function connectToken(provider: string, token: string) {
  const response = await fetch(`/api/connections/${encodeURIComponent(provider)}/token`, {
    method: 'POST', credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json', 'X-Elegy-Intent': 'user-action' },
    body: JSON.stringify({ token }),
  })
  const body = await response.json() as { message?: string }
  if (!response.ok) throw new Error(body.message ?? 'The provider could not verify this credential.')
}

export async function connectCredential(provider: string, profile: string, fields: Record<string, string>) {
  const response = await fetch(`/api/connections/${encodeURIComponent(provider)}/credential`, {
    method: 'POST', credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json', 'X-Elegy-Intent': 'user-action' },
    body: JSON.stringify({ profile, fields }),
  })
  const body = await response.json() as { message?: string }
  if (!response.ok) throw new Error(body.message ?? 'The provider could not verify this credential.')
}

async function mutate(url: string) {
  const response = await fetch(url, { method: 'POST', credentials: 'same-origin', headers: { 'X-Elegy-Intent': 'user-action' } })
  if (!response.ok) throw new Error((await response.json() as { message?: string }).message ?? 'The action could not be completed.')
}

function toAccount(account: AccountMetadata, grants: BrokerGrant[], pack?: ProviderPack): Account {
  const provider = pack?.display_name ?? titleCase(account.provider)
  return {
    id: account.id,
    provider,
    mark: provider.slice(0, 2).toUpperCase(),
    markColor: providerColor(account.provider),
    identity: account.verified_identity,
    health: 'Healthy',
    connection: connectionLabel(account.auth_method),
    connectedAt: `Connected ${new Date(account.created_at).toLocaleDateString()}`,
    grants: grants.map(toGrant),
  }
}

function toGrant(grant: BrokerGrant): Grant {
  return {
    id: grant.id,
    client: clientLabel(grant.client_id),
    summary: `Can ${grant.operations.join(', ')}.`,
    limitation: `Only for: ${grant.purpose}. Expires ${new Date(grant.expires_at).toLocaleString()}.`,
  }
}

function titleCase(value: string) { return value.charAt(0).toUpperCase() + value.slice(1).toLowerCase() }
function providerColor(value: string) { return ['#1d4ed8', '#6d28d9', '#0f766e', '#a16207'][[...value].reduce((sum, character) => sum + character.charCodeAt(0), 0) % 4] }
function connectionLabel(value: string) { return value.includes('oauth') ? 'Connected with OAuth' : value.includes('browser') ? 'Connected through browser' : 'Connected with a limited token' }
function clientLabel(value: string): Grant['client'] { return value.includes('holon') || value.includes('studio') ? 'Holon' : value.includes('brave') ? 'Browser' : 'Codex' }
