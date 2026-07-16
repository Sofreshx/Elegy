import { discoveryHintForUrl } from './security'

const title = document.querySelector<HTMLElement>('[data-title]')!
const detail = document.querySelector<HTMLElement>('[data-detail]')!
const allow = document.querySelector<HTMLButtonElement>('[data-allow]')!
void initialize()

async function initialize() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true })
  const hint = tab?.url ? discoveryHintForUrl(tab.url) : null
  if (!hint) {
    title.textContent = 'No supported account found'
    detail.textContent = 'Open Cloudflare or GitHub, then try again.'
    allow.disabled = true
    return
  }
  title.textContent = `Continue with ${providerName(hint.providerId)}`
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

function providerName(id: string) {
  return ({ cloudflare: 'Cloudflare', github: 'GitHub' } as Record<string, string>)[id] ?? id
}
