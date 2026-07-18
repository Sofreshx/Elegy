import { discoveryHintForUrl, sanitizeNativeMessage, type ProviderDescriptor } from './security'

const nativeHost = 'com.elegy.accounts'

chrome.runtime.onMessage.addListener((request, _sender, sendResponse) => {
  if (!['connect-current-tab', 'provider-registry'].includes(request?.type)) return false
  const action = request.type === 'provider-registry' ? loadRegistry() : connectCurrentTab()
  void action.then(sendResponse, error => sendResponse({ ok: false, error: safeError(error) }))
  return true
})

async function connectCurrentTab(): Promise<Record<string, unknown>> {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true })
  const registry = await loadRegistry()
  const hint = tab.url ? discoveryHintForUrl(tab.url, registry.providers as ProviderDescriptor[]) : null
  if (!hint) return { ok: false, error: 'Open a supported provider account page first.' }
  const message = sanitizeNativeMessage({ type: 'account.discovery', version: 1, hint, interaction: 'explicit-user-allow' })
  const response = await chrome.runtime.sendNativeMessage(nativeHost, message)
  const safeResponse = sanitizeNativeMessage(response)
  if (safeResponse.ok && typeof safeResponse.openCenter === 'string') {
    const center = new URL(safeResponse.openCenter)
    center.searchParams.set('connect', hint.providerId)
    center.searchParams.set('discovered', 'brave')
    await chrome.tabs.create({ url: center.toString() })
  }
  return safeResponse
}

async function loadRegistry(): Promise<{ ok: boolean; providers: ProviderDescriptor[] }> {
  const response = sanitizeNativeMessage(await chrome.runtime.sendNativeMessage(nativeHost, { type: 'account.providers', version: 1 })) as { ok?: boolean; providers?: ProviderDescriptor[]; error?: string }
  if (!response.ok || !Array.isArray(response.providers)) throw new Error(response.error ?? 'Could not load trusted provider packs.')
  return { ok: true, providers: response.providers }
}

function safeError(error: unknown): string {
  return error instanceof Error ? error.message.replace(/(Bearer|token|secret)\s+\S+/gi, '[REDACTED]') : 'Connection failed.'
}
