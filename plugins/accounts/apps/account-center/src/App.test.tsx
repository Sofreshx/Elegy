import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { App } from './App'

describe('Account Center', () => {
  it('renders an arbitrary provider supplied by the broker registry', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({
      accounts: [], requests: [], grants: [], audit: [], authorizations: [], providers: [{
        schema_version: 'elegy-account-provider/v1', id: 'synthetic-mail', display_name: 'Synthetic Mail',
        version: '1.0.0', publisher: 'fixture', browser_origins: ['https://accounts.example.test'],
        auth_profiles: [{ id: 'oauth', method: 'oauth_pkce', audience: 'https://api.example.test', identity: { url: 'https://api.example.test/me', selectors: ['/email'] }, client: { mode: 'environment' }, scopes: ['mail.read'] }],
        operations: { 'mail.read': ['mail.read'] },
      }],
    }), { status: 200 })))
    const user = userEvent.setup()
    render(<App demo={false} />)
    await user.click((await screen.findAllByRole('button', { name: 'Connect account' }))[0])
    expect(screen.getByRole('button', { name: 'Continue with Synthetic Mail' })).toBeVisible()
    vi.unstubAllGlobals()
  })

  it('shows account inventory and plain-language access without secret-shaped fields', () => {
    render(<App />)
    expect(screen.getByRole('heading', { name: 'Accounts & access' })).toBeVisible()
    expect(screen.getByRole('button', { name: 'Connect account' })).toBeEnabled()
    expect(screen.getByRole('row', { name: /Example Edge/ })).toBeVisible()
    expect(screen.getByRole('row', { name: /Code Forge/ })).toBeVisible()
    expect(screen.getByText('Can read zones, DNS records, and settings.')).toBeVisible()
    expect(document.body.textContent).not.toMatch(/access[_ -]?token|refresh[_ -]?token|api[_ -]?key|Bearer/i)
  })

  it('selects another account and updates the persistent detail pane', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: /Select Code Forge account/ }))
    const detail = screen.getByRole('complementary', { name: 'Account details' })
    expect(within(detail).getByRole('heading', { name: 'Code Forge' })).toBeVisible()
    expect(within(detail).getByText('Connected with device authorization')).toBeVisible()
  })

  it('resolves a pending request through an explicit denial', async () => {
    const user = userEvent.setup()
    render(<App />)
    expect(screen.getByText(/Codex is waiting for dns\.list access/)).toBeVisible()
    await user.click(screen.getByRole('button', { name: 'Deny request' }))
    expect(screen.queryByText(/Codex is waiting for dns\.list access/)).not.toBeInTheDocument()
  })

  it('opens provider discovery and explains the safe browser boundary', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: 'Connect account' }))
    const dialog = screen.getByRole('dialog', { name: 'Connect account' })
    expect(within(dialog).getByRole('button', { name: /Continue with Example Edge/ })).toBeVisible()
    expect(within(dialog).getByRole('button', { name: /Continue with Code Forge/ })).toBeVisible()
    expect(within(dialog).getByText(/Passwords and browser cookies are never imported/)).toBeVisible()
  })

  it('keeps provider token capture inside the trusted Account Center', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: 'Connect account' }))
    await user.click(screen.getByRole('button', { name: /Continue with Example Edge/i }))
    const dialog = screen.getByRole('dialog', { name: /Connect Example Edge/i })
    expect(within(dialog).getByLabelText(/Example Edge credential/i)).toHaveAttribute('type', 'password')
    expect(within(dialog).getByRole('link', { name: /Create scoped credential/i })).toHaveAttribute('target', '_blank')
  })

  it('uses the same account components in embedded mode without the product rail', () => {
    render(<App mode="embedded" />)
    expect(screen.queryByRole('navigation', { name: 'Account Center' })).not.toBeInTheDocument()
    expect(screen.getByRole('row', { name: /Example Edge/ })).toBeVisible()
    expect(screen.getByRole('complementary', { name: 'Account details' })).toBeVisible()
  })

  it('makes resumable account creation checkpoints actionable', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: 'Requests' }))
    expect(screen.getByRole('heading', { name: 'Code Forge' })).toBeVisible()
    expect(screen.getByRole('button', { name: 'Continue setup' })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Cancel Code Forge request' })).toBeEnabled()
  })

  it('shows a provider device code without exposing the private device credential', async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input)
      if (url === '/api/state') return new Response(JSON.stringify({ accounts: [], requests: [], grants: [], audit: [], providers: [{ schema_version: 'elegy-account-provider/v1', id: 'code-forge', display_name: 'Code Forge', version: '1.0.0', publisher: 'fixture', browser_origins: ['https://code.example.test'], auth_profiles: [{ id: 'device', method: 'device_authorization', scopes: ['profile.read'] }], operations: {} }] }), { status: 200 })
      if (url.includes('/api/connections/code-forge/start')) return new Response(JSON.stringify({
        mode: 'device', request_id: 'request-1', user_code: 'ABCD-EFGH',
        verification_uri: 'https://code.example.test/device', expires_in: 900, interval: 30,
      }), { status: 200 })
      return new Response(JSON.stringify({ status: 'pending', retry_after: 30 }), { status: 200 })
    })
    vi.stubGlobal('fetch', fetchMock)
    const user = userEvent.setup()
    render(<App demo={false} />)
    await waitFor(() => expect(screen.getAllByRole('button', { name: 'Connect account' })[0]).toBeEnabled())
    await user.click(screen.getAllByRole('button', { name: 'Connect account' })[0])
    await user.click(screen.getByRole('button', { name: /Continue with Code Forge/ }))
    const dialog = await screen.findByRole('dialog', { name: 'Authorize Code Forge' })
    expect(within(dialog).getByText('ABCD-EFGH')).toBeVisible()
    expect(within(dialog).getByRole('link', { name: 'Open Code Forge' })).toHaveAttribute('href', 'https://code.example.test/device')
    expect(document.body.textContent).not.toContain('request-1')
    vi.unstubAllGlobals()
  })

  it('restores a pending authorization from broker state after the UI is reopened', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({
      accounts: [], requests: [], grants: [], audit: [], authorizations: [{
        id: 'auth-1', provider: 'code-forge', status: 'waiting_for_user', user_code: 'WXYZ-1234',
        verification_uri: 'https://code.example.test/device', expires_at: '2099-01-01T00:00:00Z',
        interval_seconds: 5, next_poll_at: '2098-12-31T23:59:00Z', created_at: '2098-12-31T23:58:00Z', updated_at: '2098-12-31T23:58:00Z',
      }], providers: [{ schema_version: 'elegy-account-provider/v1', id: 'code-forge', display_name: 'Code Forge', version: '1.0.0', publisher: 'fixture', browser_origins: [], auth_profiles: [], operations: {} }],
    }), { status: 200 })))
    render(<App demo={false} />)
    expect(await screen.findByText(/Code Forge authorization is waiting for you/)).toBeVisible()
    expect(screen.getByRole('button', { name: 'Review Code Forge authorization' })).toBeEnabled()
    vi.unstubAllGlobals()
  })

  it('offers an explicit retry when a provider authorization expired while the user was away', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({
      accounts: [], requests: [], grants: [], audit: [], authorizations: [{
        id: 'auth-2', provider: 'code-forge', status: 'interaction_required', user_code: '',
        verification_uri: 'https://code.example.test/device', expires_at: '2020-01-01T00:00:00Z',
        interval_seconds: 5, next_poll_at: '2020-01-01T00:00:00Z', last_error: 'expired_token', created_at: '2020-01-01T00:00:00Z', updated_at: '2020-01-01T00:00:00Z',
      }], providers: [{ schema_version: 'elegy-account-provider/v1', id: 'code-forge', display_name: 'Code Forge', version: '1.0.0', publisher: 'fixture', browser_origins: [], auth_profiles: [], operations: {} }],
    }), { status: 200 })))
    render(<App demo={false} />)
    expect(await screen.findByText(/Code Forge authorization expired/)).toBeVisible()
    expect(screen.getByRole('button', { name: 'Retry Code Forge authorization' })).toBeEnabled()
    vi.unstubAllGlobals()
  })
})
