import { AlertTriangle, Plus, ShieldCheck } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useEffect } from 'react'
import { AccountDetail } from './components/AccountDetail'
import { AccountInventory } from './components/AccountInventory'
import { ConfirmDialog, ConnectDialog, CredentialDialog, DeviceFlowDialog } from './components/Dialogs'
import { SideNav, type View } from './components/SideNav'
import { seedAccounts, seedProviders } from './types'
import { approveRequest, cancelRequest, connectCredential, disconnectAccount, loadState, startConnection, type AccountState, type AuthorizationSession, type BrokerRequest, type ConnectionStart, type CredentialField } from './api'
import './app.css'

export function App({ mode = 'standalone', demo = import.meta.env.DEV }: { mode?: 'standalone' | 'embedded'; demo?: boolean }) {
  const [view, setView] = useState<View>('accounts')
  const [state, setState] = useState<AccountState>({ accounts: demo ? seedAccounts : [], requests: demo ? [
    { id: 'demo-request', kind: 'access', status: 'awaiting_user', account_id: seedAccounts[0].id, provider: 'example-edge', client_id: 'codex-local', purpose: 'read-only DNS research', operations: ['dns.list'], duration_minutes: 60 },
    { id: 'demo-creation', kind: 'creation', status: 'waiting_human', provider: 'code-forge', purpose: 'publish a new project', operations: ['free plan'], duration_minutes: 0 },
  ] : [], audit: [], authorizations: [], providers: demo ? seedProviders : [] })
  const [selectedId, setSelectedId] = useState(demo ? seedAccounts[0].id : '')
  const [compact, setCompact] = useState(false)
  const [detailOpen, setDetailOpen] = useState(false)
  const [error, setError] = useState('')
  const [deviceFlow, setDeviceFlow] = useState<Extract<ConnectionStart, { mode: 'device' }> | null>(null)
  const [credentialConnection, setCredentialConnection] = useState<{ provider: string; profile: string; creationUrl?: string; fields: CredentialField[] } | null>(null)
  const initialConnect = new URLSearchParams(window.location.search).has('connect')
  const [dialog, setDialog] = useState<'connect' | 'token' | 'review' | 'revoke' | null>(initialConnect ? 'connect' : null)
  const selected = useMemo(() => state.accounts.find(account => account.id === selectedId) ?? state.accounts[0], [state.accounts, selectedId])
  const pendingRequest = state.requests.find(request => request.status === 'awaiting_user')
  const activeAuthorization = [...state.authorizations].reverse().find(session => ['waiting_for_user', 'interaction_required'].includes(session.status))
  const providerName = (provider: string) => state.providers.find(candidate => candidate.id === provider)?.display_name ?? providerDisplayName(provider)

  const refresh = async (signal?: AbortSignal) => {
    if (demo) return
    try {
      const next = await loadState(signal)
      setState(next)
      setSelectedId(current => next.accounts.some(account => account.id === current) ? current : (next.accounts[0]?.id ?? ''))
      setError('')
    } catch (caught) {
      if (!signal?.aborted) setError(caught instanceof Error ? caught.message : 'The local account broker is unavailable.')
    }
  }

  useEffect(() => {
    const controller = new AbortController()
    void refresh(controller.signal)
    const timer = demo ? undefined : window.setInterval(() => void refresh(), 2000)
    return () => { controller.abort(); if (timer) window.clearInterval(timer) }
  }, [demo])
  useEffect(() => {
    const query = window.matchMedia?.('(max-width: 620px)')
    if (!query) return
    const update = () => setCompact(query.matches)
    update(); query.addEventListener('change', update)
    return () => query.removeEventListener('change', update)
  }, [])
  const revokeSelected = () => {
    if (!selected) return
    if (demo) {
      const remaining = state.accounts.filter(account => account.id !== selected.id)
      setState(current => ({ ...current, accounts: remaining }))
      setSelectedId(remaining[0]?.id ?? '')
      setDialog(null)
      return
    }
    void disconnectAccount(selected.id).then(() => refresh()).then(() => setDialog(null)).catch(caught => setError(caught.message))
  }

  const approvePending = () => {
    if (!pendingRequest) return
    if (demo) { setState(current => ({ ...current, requests: current.requests.map(request => request.id === pendingRequest.id ? { ...request, status: 'approved' } : request) })); setDialog(null); return }
    void approveRequest(pendingRequest.id).then(() => refresh()).then(() => setDialog(null)).catch(caught => setError(caught.message))
  }

  const cancelBrokerRequest = (request: BrokerRequest) => {
    if (demo) { setState(current => ({ ...current, requests: current.requests.filter(candidate => candidate.id !== request.id) })); return }
    void cancelRequest(request.id).then(() => refresh()).catch(caught => setError(caught.message))
  }
  const denyPending = () => pendingRequest && cancelBrokerRequest(pendingRequest)
  const connectProvider = (provider: string) => {
    if (demo) {
      const profile = state.providers.find(candidate => candidate.id === provider)?.auth_profiles[0]
      if (profile?.method === 'api_token') { setCredentialConnection({ provider, profile: profile.id, creationUrl: profile.creation_url, fields: profile.credential_fields ?? [{ id: 'token', label: `${providerName(provider)} credential`, secret: true, autocomplete: 'off' }] }); setDialog('token') }
      else setDialog(null)
      return
    }
    setError('')
    void startConnection(provider).then(async connection => {
      setDialog(null)
      if (connection.mode === 'browser') window.location.assign(connection.authorization_url)
      else if (connection.mode === 'manual_credential') { setCredentialConnection({ provider: connection.provider, profile: connection.profile, creationUrl: connection.creation_url, fields: connection.credential_fields }); setDialog('token') }
      else { await refresh(); setDeviceFlow(connection) }
    }).catch(caught => { setDialog(null); setError(caught.message) })
  }
  const submitCredential = async (fields: Record<string, string>) => {
    if (!credentialConnection) return
    if (!demo) { await connectCredential(credentialConnection.provider, credentialConnection.profile, fields); await refresh() }
    setDialog(null)
  }

  return (
    <div className={`app-shell ${mode}`}>
      {mode === 'standalone' && <SideNav view={view} onChange={setView} />}
      <main className="workspace">
        {view === 'accounts' && <>
          <header className="page-header"><div><h1>Accounts &amp; access</h1><p>Connect your online accounts and control what local AI agents can do on your behalf.</p></div><button className="primary-button" type="button" onClick={() => setDialog('connect')}><Plus />Connect account</button></header>
          <div className="account-layout">
            <div className="main-column">
              {error && <section className="broker-error" role="alert">{error}</section>}
              {activeAuthorization && <AuthorizationAttention providerName={providerName(activeAuthorization.provider)} session={activeAuthorization} onReview={() => setDeviceFlow(toConnection(activeAuthorization))} onRetry={() => connectProvider(activeAuthorization.provider)} />}
              {pendingRequest && <section className="attention" aria-label="Pending request"><AlertTriangle /><strong>{pendingRequest.client_id?.startsWith('codex') ? 'Codex' : 'A local agent'} is waiting for {pendingRequest.operations.join(', ')} access</strong><div><button type="button" className="review-button" onClick={() => setDialog('review')}>Review</button><button type="button" className="quiet-button" aria-label="Deny request" onClick={denyPending}>Deny</button></div></section>}
              <AccountInventory accounts={state.accounts} selectedId={selectedId} onSelect={id => { setSelectedId(id); setDetailOpen(true) }} />
              {!state.accounts.length && <EmptyAccounts onConnect={() => setDialog('connect')} />}
            </div>
            {selected && (!compact || detailOpen) && <AccountDetail account={selected} onReview={() => setDialog('review')} onRevoke={() => setDialog('revoke')} onClose={compact ? () => setDetailOpen(false) : undefined} />}
          </div>
        </>}
        {view === 'requests' && <RequestsView providers={state.providers} requests={state.requests} onContinue={connectProvider} onCancel={cancelBrokerRequest} />}
        {view === 'activity' && <SimpleView title="Activity" copy={`${state.audit.length} sanitized event${state.audit.length === 1 ? '' : 's'} recorded locally. Credential values are never included.`} />}
      </main>
      {dialog === 'connect' && <ConnectDialog providers={state.providers} onClose={() => setDialog(null)} onProvider={connectProvider} />}
      {dialog === 'token' && credentialConnection && <CredentialDialog providerName={providerName(credentialConnection.provider)} creationUrl={credentialConnection.creationUrl} fields={credentialConnection.fields} onSubmit={submitCredential} onClose={() => setDialog(null)} />}
      {dialog === 'review' && pendingRequest && <ConfirmDialog title={`${pendingRequest.operations.join(', ')} access`} copy={`${pendingRequest.client_id ?? 'A local agent'} may use only the listed operations for “${pendingRequest.purpose}” for ${pendingRequest.duration_minutes} minutes. It receives an opaque, revocable lease—not the credential.`} confirmLabel={`Allow for ${pendingRequest.duration_minutes} minutes`} onConfirm={approvePending} onClose={() => setDialog(null)} />}
      {dialog === 'revoke' && selected && <ConfirmDialog title={`Revoke ${selected.provider}?`} copy="All local grants and active leases for this account stop immediately. Elegy will also attempt provider revocation where supported." confirmLabel="Revoke account" destructive onConfirm={revokeSelected} onClose={() => setDialog(null)} />}
      {deviceFlow && <DeviceFlowDialog providerName={providerName(deviceFlow.provider)} code={deviceFlow.user_code} verificationUri={deviceFlow.verification_uri} expiresAt={activeAuthorization?.expires_at} onClose={() => setDeviceFlow(null)} />}
    </div>
  )
}

function AuthorizationAttention({ providerName, session, onReview, onRetry }: { providerName: string; session: AuthorizationSession; onReview: () => void; onRetry: () => void }) {
  const provider = providerName
  const expired = session.status === 'interaction_required'
  return <section className={`attention authorization-attention ${expired ? 'expired' : ''}`} aria-label={`${provider} authorization`}><AlertTriangle /><span><strong>{expired ? `${provider} authorization expired` : `${provider} authorization is waiting for you`}</strong><small>{expired ? 'Nothing was granted. Retry when you are ready to interact.' : 'You can close this window and return—the local broker keeps the request alive.'}</small></span><div>{expired ? <button type="button" className="review-button" aria-label={`Retry ${provider} authorization`} onClick={onRetry}>Retry now</button> : <button type="button" className="review-button" aria-label={`Review ${provider} authorization`} onClick={onReview}>Review</button>}</div></section>
}

function toConnection(session: AuthorizationSession): Extract<ConnectionStart, { mode: 'device' }> {
  return { mode: 'device', provider: session.provider, request_id: session.id, user_code: session.user_code, verification_uri: session.verification_uri, expires_in: Math.max(0, Math.floor((new Date(session.expires_at).getTime() - Date.now()) / 1000)), interval: session.interval_seconds }
}

function EmptyAccounts({ onConnect }: { onConnect: () => void }) {
  return <section className="empty-state"><ShieldCheck /><h2>No connected accounts</h2><p>Connect an account to grant agents limited, revocable access.</p><button className="primary-button" type="button" onClick={onConnect}>Connect account</button></section>
}

function SimpleView({ title, copy }: { title: string; copy: string }) {
  return <section className="simple-view"><h1>{title}</h1><p>{copy}</p><div className="empty-line">Nothing needs attention.</div></section>
}

function RequestsView({ providers, requests, onContinue, onCancel }: { providers: AccountState['providers']; requests: BrokerRequest[]; onContinue: (provider: string) => void; onCancel: (request: BrokerRequest) => void }) {
  const active = requests.filter(request => !['approved', 'cancelled'].includes(request.status))
  return <section className="requests-view"><h1>Requests</h1><p>Account access and creation checkpoints stay here until you decide.</p>{active.length ? <div className="request-list">{active.map(request => { const label = request.provider ? providers.find(provider => provider.id === request.provider)?.display_name ?? providerDisplayName(request.provider) : 'Connected account'; return <article className="request-card" key={request.id}><div><span className="request-kind">{request.kind === 'creation' ? 'Account setup' : 'Access request'}</span><h2>{label}</h2><p>{request.purpose}</p><small>Status: {request.status.replace('_', ' ')}{request.operations.length ? ` · ${request.operations.join(', ')}` : ''}</small></div><div>{request.kind === 'creation' && request.provider && <button className="primary-button" type="button" onClick={() => onContinue(request.provider!)}>Continue setup</button>}<button className="quiet-button" type="button" aria-label={`Cancel ${label} request`} onClick={() => onCancel(request)}>Cancel request</button></div></article> })}</div> : <div className="empty-line">Nothing needs attention.</div>}</section>
}

function providerDisplayName(provider: string) { return provider.split('-').map(part => `${part.charAt(0).toUpperCase()}${part.slice(1)}`).join(' ') }
