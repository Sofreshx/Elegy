import { Activity, KeyRound, ListChecks, ShieldCheck } from 'lucide-react'

export type View = 'accounts' | 'requests' | 'activity'

export function SideNav({ view, onChange }: { view: View; onChange: (view: View) => void }) {
  return (
    <nav className="side-nav" aria-label="Account Center">
      <div className="brand"><span className="brand-mark"><ShieldCheck /></span><span>Elegy Accounts</span></div>
      <div className="nav-items">
        <NavItem active={view === 'accounts'} icon={<KeyRound />} label="Accounts" onClick={() => onChange('accounts')} />
        <NavItem active={view === 'requests'} icon={<ListChecks />} label="Requests" onClick={() => onChange('requests')} />
        <NavItem active={view === 'activity'} icon={<Activity />} label="Activity" onClick={() => onChange('activity')} />
      </div>
      <div className="local-status"><span className="status-dot" /> <strong>Local only</strong><small>All data stays on this device</small></div>
    </nav>
  )
}

function NavItem({ active, icon, label, onClick }: { active: boolean; icon: React.ReactNode; label: string; onClick: () => void }) {
  return <button type="button" aria-label={label} className={active ? 'nav-item active' : 'nav-item'} onClick={onClick}>{icon}<span>{label}</span></button>
}
