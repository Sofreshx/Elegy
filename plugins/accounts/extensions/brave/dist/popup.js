// src/security.ts
var providerByOrigin = /* @__PURE__ */ new Map([
  ["https://dash.cloudflare.com", "cloudflare"],
  ["https://github.com", "github"]
]);
function discoveryHintForUrl(rawUrl) {
  let url;
  try {
    url = new URL(rawUrl);
  } catch {
    return null;
  }
  const providerId = providerByOrigin.get(url.origin);
  return providerId ? { providerId, origin: url.origin, verified: false } : null;
}

// src/popup.ts
var title = document.querySelector("[data-title]");
var detail = document.querySelector("[data-detail]");
var allow = document.querySelector("[data-allow]");
void initialize();
async function initialize() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  const hint = tab?.url ? discoveryHintForUrl(tab.url) : null;
  if (!hint) {
    title.textContent = "No supported account found";
    detail.textContent = "Open Cloudflare or GitHub, then try again.";
    allow.disabled = true;
    return;
  }
  title.textContent = `Continue with ${providerName(hint.providerId)}`;
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
function providerName(id) {
  return { cloudflare: "Cloudflare", github: "GitHub" }[id] ?? id;
}
