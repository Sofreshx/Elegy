// src/security.ts
function discoveryHintForUrl(rawUrl, providers) {
  let url;
  try {
    url = new URL(rawUrl);
  } catch {
    return null;
  }
  const providerByOrigin = new Map(providers.flatMap((provider) => provider.browserOrigins.map((origin) => [origin, provider.id])));
  const providerId = providerByOrigin.get(url.origin);
  return providerId ? { providerId, origin: url.origin, verified: false } : null;
}

// src/popup.ts
var title = document.querySelector("[data-title]");
var detail = document.querySelector("[data-detail]");
var allow = document.querySelector("[data-allow]");
void initialize();
async function initialize() {
  const registry = await chrome.runtime.sendMessage({ type: "provider-registry" });
  if (!registry?.ok || !registry.providers) {
    title.textContent = "Account broker unavailable";
    detail.textContent = registry?.error ?? "Start Elegy Account Center and try again.";
    allow.disabled = true;
    return;
  }
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  const hint = tab?.url ? discoveryHintForUrl(tab.url, registry.providers) : null;
  if (!hint) {
    title.textContent = "No supported account found";
    detail.textContent = "Open a page for one of your installed provider packs, then try again.";
    allow.disabled = true;
    return;
  }
  title.textContent = `Continue with ${registry.providers.find((provider) => provider.id === hint.providerId)?.displayName ?? hint.providerId}`;
  detail.textContent = "Elegy will open a provider-approved connection flow. Passwords and cookies are never imported.";
  allow.addEventListener("click", async () => {
    allow.disabled = true;
    allow.textContent = "Opening\u2026";
    const response = await chrome.runtime.sendMessage({ type: "connect-current-tab" });
    if (!response?.ok) {
      detail.textContent = response?.error ?? "Could not contact the local broker.";
      allow.disabled = false;
      allow.textContent = "Allow";
      return;
    }
    title.textContent = "Continue in Account Center";
    detail.textContent = "The local broker is ready to verify this account.";
    allow.hidden = true;
  });
}
