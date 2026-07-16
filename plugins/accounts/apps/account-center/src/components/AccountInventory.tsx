import { CheckCircle2, ChevronRight } from 'lucide-react'
import type { Account } from '../types'

export function AccountInventory({ accounts, selectedId, onSelect }: { accounts: Account[]; selectedId: string; onSelect: (id: string) => void }) {
  return (
    <section className="inventory" aria-labelledby="accounts-title">
      <h2 id="accounts-title">Your accounts</h2>
      <div className="table-wrap">
        <table>
          <thead><tr><th>Account</th><th>Verified identity</th><th>Connection</th><th>Agent access</th><th><span className="sr-only">Select</span></th></tr></thead>
          <tbody>
            {accounts.map(account => (
              <tr key={account.id} className={account.id === selectedId ? 'selected' : undefined}>
                <td><button type="button" className="account-select" aria-label={`Select ${account.provider} account`} onClick={() => onSelect(account.id)}><ProviderMark account={account} /><strong>{account.provider}</strong></button></td>
                <td><span className="identity"><CheckCircle2 />{account.identity}</span></td>
                <td><span className={`health ${account.health.toLowerCase()}`}><span />{account.health}</span></td>
                <td>{accessSummary(account)}</td>
                <td><button type="button" className="row-arrow" aria-label={`Open ${account.provider} details`} onClick={() => onSelect(account.id)}><ChevronRight /></button></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  )
}

export function ProviderMark({ account }: { account: Account }) {
  return <span className="provider-mark" style={{ '--mark-color': account.markColor } as React.CSSProperties} aria-hidden="true">{account.mark}</span>
}

function accessSummary(account: Account) {
  return account.grants.map(grant => `${grant.client} (read)`).join(', ')
}
