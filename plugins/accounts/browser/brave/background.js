// src/security.ts
var secretKeys = /* @__PURE__ */ new Set([
  "authorization",
  "password",
  "cookie",
  "set-cookie",
  "access_token",
  "refresh_token",
  "api_key",
  "client_secret",
  "secret",
  "token"
]);
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
function sanitizeNativeMessage(message) {
  assertNoSecretFields(message, "$");
  return structuredClone(message);
}
function assertNoSecretFields(value, path) {
  if (!value || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    if (secretKeys.has(key.toLowerCase())) throw new Error(`Secret-bearing field rejected at ${path}.${key}`);
    assertNoSecretFields(child, `${path}.${key}`);
  }
}

// src/background.ts
var nativeHost = "com.elegy.accounts";
chrome.runtime.onMessage.addListener((request, _sender, sendResponse) => {
  if (!["connect-current-tab", "provider-registry"].includes(request?.type)) return false;
  const action = request.type === "provider-registry" ? loadRegistry() : connectCurrentTab();
  void action.then(sendResponse, (error) => sendResponse({ ok: false, error: safeError(error) }));
  return true;
});
async function connectCurrentTab() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  const registry = await loadRegistry();
  const hint = tab.url ? discoveryHintForUrl(tab.url, registry.providers) : null;
  if (!hint) return { ok: false, error: "Open a supported provider account page first." };
  const message = sanitizeNativeMessage({ type: "account.discovery", version: 1, hint, interaction: "explicit-user-allow" });
  const response = await chrome.runtime.sendNativeMessage(nativeHost, message);
  const safeResponse = sanitizeNativeMessage(response);
  if (safeResponse.ok && typeof safeResponse.openCenter === "string") {
    const center = new URL(safeResponse.openCenter);
    center.searchParams.set("connect", hint.providerId);
    center.searchParams.set("discovered", "brave");
    await chrome.tabs.create({ url: center.toString() });
  }
  return safeResponse;
}
async function loadRegistry() {
  const response = sanitizeNativeMessage(await chrome.runtime.sendNativeMessage(nativeHost, { type: "account.providers", version: 1 }));
  if (!response.ok || !Array.isArray(response.providers)) throw new Error(response.error ?? "Could not load trusted provider packs.");
  return { ok: true, providers: response.providers };
}
function safeError(error) {
  return error instanceof Error ? error.message.replace(/(Bearer|token|secret)\s+\S+/gi, "[REDACTED]") : "Connection failed.";
}
