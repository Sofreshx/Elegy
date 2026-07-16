import { AppWindow, Box, CheckCircle2, Eye, Globe2, Trash2, X } from 'lucide-react'
import type { Account } from '../types'
import { ProviderMark } from './AccountInventory'

export function AccountDetail({ account, onReview, onRevoke, onClose }: { account: Account; onReview: () => void; onRevoke: () => void; onClose?: () => void }) {
  return (
    <aside className="detail-pane" aria-label="Account details">
      <header><div className="detail-title"><ProviderMark account={account} /><h2>{account.provider}</h2></div>{onClose && <button className="icon-button" type="button" aria-label="Close account details" onClick={onClose}><X /></button>}</header>
      <section className="detail-section"><p className="section-label">Verified identity</p><p className="detail-line verified"><CheckCircle2 />{account.identity}</p></section>
      <section className="detail-section"><p className="section-label">Connection</p><p className="detail-line"><Globe2 /><span><strong>{account.connection}</strong><small>{account.connectedAt}</small></span></p></section>
      <section className="detail-section access-section">
        <p className="section-label">Agent access</p>
        <p className="section-copy">These local AI agents have been granted access to this account.</p>
        {account.grants.map(grant => <div className="grant" key={grant.id}>{grant.client === 'Browser' ? <AppWindow /> : <Box />}<span><strong>{grant.client}</strong><small>{grant.summary}</small><small>{grant.limitation}</small></span></div>)}
      </section>
      <div className="detail-actions"><button className="secondary-action" type="button" onClick={onReview}><Eye />Review access</button><button className="danger-action" type="button" onClick={onRevoke}><Trash2 />Revoke account</button></div>
    </aside>
  )
}
