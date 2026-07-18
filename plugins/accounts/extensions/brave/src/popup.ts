import { discoveryHintForUrl, type ProviderDescriptor } from './security'

const title = document.querySelector<HTMLElement>('[data-title]')!
const detail = document.querySelector<HTMLElement>('[data-detail]')!
const allow = document.querySelector<HTMLButtonElement>('[data-allow]')!
void initialize()

async function initialize() {
  const registry = await chrome.runtime.sendMessage({ type: 'provider-registry' }) as { ok?: boolean; providers?: ProviderDescriptor[]; error?: string }
  if (!registry?.ok || !registry.providers) {
    title.textContent = 'Account broker unavailable'
    detail.textContent = registry?.error ?? 'Start Elegy Account Center and try again.'
    allow.disabled = true
    return
  }
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true })
  const hint = tab?.url ? discoveryHintForUrl(tab.url, registry.providers) : null
  if (!hint) {
    title.textContent = 'No supported account found'
    detail.textContent = 'Open a page for one of your installed provider packs, then try again.'
    allow.disabled = true
    return
  }
  title.textContent = `Continue with ${registry.providers.find(provider => provider.id === hint.providerId)?.displayName ?? hint.providerId}`
  detail.textContent = 'Elegy will open a provider-approved connection flow. Passwords and cookies are never imported.'
  allow.addEventListener('click', async () => {
    allow.disabled = true
    allow.textContent = 'Opening…'
    const response = await chrome.runtime.sendMessage({ type: 'connect-current-tab' })
    if (!response?.ok) {
      detail.textContent = response?.error ?? 'Could not contact the local broker.'
      allow.disabled = false
      allow.textContent = 'Allow'
      return
    }
    title.textContent = 'Continue in Account Center'
    detail.textContent = 'The local broker is ready to verify this account.'
    allow.hidden = true
  })
}
