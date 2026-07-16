import { ArrowRight, Cloud, Github, ShieldCheck, X } from 'lucide-react'
import { useState } from 'react'

export function ConnectDialog({ onClose, onProvider = () => undefined }: { onClose: () => void; onProvider?: (provider: string) => void }) {
  return (
    <div className="dialog-backdrop" role="presentation">
      <section className="dialog" role="dialog" aria-modal="true" aria-labelledby="connect-title">
        <header><div><p className="section-label">Account discovery</p><h2 id="connect-title">Connect account</h2></div><button className="icon-button" type="button" aria-label="Close connect account" onClick={onClose}><X /></button></header>
        <p className="dialog-copy">Continue with a signed-in browser or a provider-approved authorization flow. Passwords and browser cookies are never imported.</p>
        <div className="provider-options">
          <ProviderOption name="Cloudflare" icon={<Cloud />} note="Guided scoped API token" onSelect={onProvider} />
          <ProviderOption name="GitHub" icon={<Github />} note="Secure device flow" onSelect={onProvider} />
        </div>
        <div className="safety-note"><ShieldCheck /><span><strong>The agent never sees the credential.</strong><small>Elegy stores it locally and returns only a scoped capability.</small></span></div>
      </section>
    </div>
  )
}

function ProviderOption({ name, icon, note, onSelect }: { name: string; icon: React.ReactNode; note: string; onSelect: (provider: string) => void }) {
  return <button type="button" aria-label={`Continue with ${name}`} onClick={() => onSelect(name.toLowerCase())}><span className="option-icon">{icon}</span><span><strong>{name}</strong><small>{note}</small></span><ArrowRight /></button>
}

export function ConfirmDialog({ title, copy, confirmLabel, destructive, onConfirm, onClose }: { title: string; copy: string; confirmLabel: string; destructive?: boolean; onConfirm: () => void; onClose: () => void }) {
  return <div className="dialog-backdrop" role="presentation"><section className="dialog confirm" role="dialog" aria-modal="true" aria-labelledby="confirm-title"><h2 id="confirm-title">{title}</h2><p className="dialog-copy">{copy}</p><div className="confirm-actions"><button type="button" className="quiet-button" onClick={onClose}>Cancel</button><button type="button" className={destructive ? 'solid-danger' : 'primary-button'} onClick={onConfirm}>{confirmLabel}</button></div></section></div>
}

export function DeviceFlowDialog({ code, verificationUri, expiresAt, onClose }: { code: string; verificationUri: string; expiresAt?: string; onClose: () => void }) {
  const minutes = expiresAt ? Math.max(1, Math.ceil((new Date(expiresAt).getTime() - Date.now()) / 60000)) : undefined
  return <div className="dialog-backdrop" role="presentation"><section className="dialog device-flow" role="dialog" aria-modal="true" aria-labelledby="device-title"><header><div><p className="section-label">Secure device authorization</p><h2 id="device-title">Authorize GitHub</h2></div><button className="icon-button" type="button" aria-label="Close GitHub authorization" onClick={onClose}><X /></button></header><p className="dialog-copy">Open GitHub, confirm the code below, and approve the read-only profile permission. Elegy receives the resulting credential directly; the agent never sees it.</p><div className="device-code" aria-label="GitHub device code">{code}</div><a className="primary-button device-link" href={verificationUri} target="_blank" rel="noreferrer">Open GitHub</a><div className="device-wait"><span className="status-dot" />Waiting for GitHub authorization{minutes ? ` · about ${minutes} min remaining` : '…'}</div><p className="device-resume">Safe to close. This request stays in Account Center and can be reopened from any local Elegy surface.</p></section></div>
}

export function CloudflareTokenDialog({ creationUrl, onSubmit, onClose }: { creationUrl: string; onSubmit: (token: string) => Promise<void>; onClose: () => void }) {
  const [token, setToken] = useState('')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const submit = async (event: React.FormEvent) => {
    event.preventDefault(); setBusy(true); setError('')
    try { await onSubmit(token); setToken('') } catch (caught) { setError(caught instanceof Error ? caught.message : 'Cloudflare could not verify this token.'); setBusy(false) }
  }
  return <div className="dialog-backdrop" role="presentation"><section className="dialog" role="dialog" aria-modal="true" aria-labelledby="cloudflare-title"><header><div><p className="section-label">Guided scoped token</p><h2 id="cloudflare-title">Connect Cloudflare</h2></div><button className="icon-button" type="button" aria-label="Close Cloudflare connection" onClick={onClose}><X /></button></header><p className="dialog-copy">Create a token limited to the zones and read operations you need. Paste it here once; Elegy verifies and encrypts it locally. The token is never shown to an agent.</p><a className="secondary-action token-create-link" href={creationUrl} target="_blank" rel="noreferrer">Create scoped token <ArrowRight /></a><form className="token-form" onSubmit={submit}><label htmlFor="cloudflare-token">Cloudflare API token</label><input id="cloudflare-token" type="password" value={token} required autoComplete="off" spellCheck={false} onChange={event => setToken(event.target.value)} /><small>Use a scoped API token, never the Global API Key.</small>{error && <p role="alert">{error}</p>}<button className="primary-button" type="submit" disabled={busy || !token.trim()}>{busy ? 'Verifying…' : 'Verify and connect'}</button></form></section></div>
}
