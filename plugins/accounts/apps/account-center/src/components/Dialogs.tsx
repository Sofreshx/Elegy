import { ArrowRight, KeyRound, ShieldCheck, X } from 'lucide-react'
import { useState } from 'react'
import type { CredentialField, ProviderPack } from '../api'

export function ConnectDialog({ providers, onClose, onProvider = () => undefined }: { providers: ProviderPack[]; onClose: () => void; onProvider?: (provider: string) => void }) {
  return (
    <div className="dialog-backdrop" role="presentation">
      <section className="dialog" role="dialog" aria-modal="true" aria-labelledby="connect-title">
        <header><div><p className="section-label">Account discovery</p><h2 id="connect-title">Connect account</h2></div><button className="icon-button" type="button" aria-label="Close connect account" onClick={onClose}><X /></button></header>
        <p className="dialog-copy">Continue with a signed-in browser or a provider-approved authorization flow. Passwords and browser cookies are never imported.</p>
        <div className="provider-options">
          {providers.map(provider => <ProviderOption key={provider.id} provider={provider} onSelect={onProvider} />)}
          {!providers.length ? <p className="empty-line">No trusted provider packs are installed.</p> : null}
        </div>
        <div className="safety-note"><ShieldCheck /><span><strong>The agent never sees the credential.</strong><small>Elegy stores it locally and returns only a scoped capability.</small></span></div>
      </section>
    </div>
  )
}

function ProviderOption({ provider, onSelect }: { provider: ProviderPack; onSelect: (provider: string) => void }) {
  const method = provider.auth_profiles[0]?.method
  const note = ({ oauth_pkce: 'Secure browser authorization', device_authorization: 'Secure device authorization', api_token: 'Guided scoped token', http_basic: 'App password or Basic credential', client_credentials: 'Service client credential', service_credential: 'Provider service credential' } as Record<string, string>)[method] ?? 'Provider-approved connection'
  return <button type="button" aria-label={`Continue with ${provider.display_name}`} onClick={() => onSelect(provider.id)}><span className="option-icon"><KeyRound /></span><span><strong>{provider.display_name}</strong><small>{note}</small></span><ArrowRight /></button>
}

export function ConfirmDialog({ title, copy, confirmLabel, destructive, onConfirm, onClose }: { title: string; copy: string; confirmLabel: string; destructive?: boolean; onConfirm: () => void; onClose: () => void }) {
  return <div className="dialog-backdrop" role="presentation"><section className="dialog confirm" role="dialog" aria-modal="true" aria-labelledby="confirm-title"><h2 id="confirm-title">{title}</h2><p className="dialog-copy">{copy}</p><div className="confirm-actions"><button type="button" className="quiet-button" onClick={onClose}>Cancel</button><button type="button" className={destructive ? 'solid-danger' : 'primary-button'} onClick={onConfirm}>{confirmLabel}</button></div></section></div>
}

export function DeviceFlowDialog({ providerName, code, verificationUri, expiresAt, onClose }: { providerName: string; code: string; verificationUri: string; expiresAt?: string; onClose: () => void }) {
  const minutes = expiresAt ? Math.max(1, Math.ceil((new Date(expiresAt).getTime() - Date.now()) / 60000)) : undefined
  return <div className="dialog-backdrop" role="presentation"><section className="dialog device-flow" role="dialog" aria-modal="true" aria-labelledby="device-title"><header><div><p className="section-label">Secure device authorization</p><h2 id="device-title">Authorize {providerName}</h2></div><button className="icon-button" type="button" aria-label={`Close ${providerName} authorization`} onClick={onClose}><X /></button></header><p className="dialog-copy">Open {providerName}, confirm the code below, and approve the requested scopes. Elegy receives the credential directly; the agent never sees it.</p><div className="device-code" aria-label={`${providerName} device code`}>{code}</div><a className="primary-button device-link" href={verificationUri} target="_blank" rel="noreferrer">Open {providerName}</a><div className="device-wait"><span className="status-dot" />Waiting for {providerName} authorization{minutes ? ` · about ${minutes} min remaining` : '…'}</div><p className="device-resume">Safe to close. This request stays in Account Center and can be reopened from any local Elegy surface.</p></section></div>
}

export function CredentialDialog({ providerName, creationUrl, fields, onSubmit, onClose }: { providerName: string; creationUrl?: string; fields: CredentialField[]; onSubmit: (values: Record<string, string>) => Promise<void>; onClose: () => void }) {
  const [values, setValues] = useState<Record<string, string>>({})
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const submit = async (event: React.FormEvent) => {
    event.preventDefault(); setBusy(true); setError('')
    try { await onSubmit(values); setValues({}) } catch (caught) { setError(caught instanceof Error ? caught.message : 'The provider could not verify this credential.'); setBusy(false) }
  }
  const complete = fields.length > 0 && fields.every(field => values[field.id]?.trim())
  return <div className="dialog-backdrop" role="presentation"><section className="dialog" role="dialog" aria-modal="true" aria-labelledby="token-title"><header><div><p className="section-label">Guided scoped credential</p><h2 id="token-title">Connect {providerName}</h2></div><button className="icon-button" type="button" aria-label={`Close ${providerName} connection`} onClick={onClose}><X /></button></header><p className="dialog-copy">Enter the narrowest credential required for your task. Elegy verifies and encrypts it locally. The agent never sees it.</p>{creationUrl ? <a className="secondary-action token-create-link" href={creationUrl} target="_blank" rel="noreferrer">Create scoped credential <ArrowRight /></a> : null}<form className="token-form" onSubmit={submit}>{fields.map(field => <label key={field.id}>{field.label}<input name={field.id} type={field.secret ? 'password' : 'text'} value={values[field.id] ?? ''} required autoComplete={field.autocomplete ?? 'off'} spellCheck={false} onChange={event => setValues(current => ({ ...current, [field.id]: event.target.value }))} /></label>)}<small>Use a limited credential rather than a global or owner key.</small>{error ? <p role="alert">{error}</p> : null}<button className="primary-button" type="submit" disabled={busy || !complete}>{busy ? 'Verifying…' : 'Verify and connect'}</button></form></section></div>
}
