# Windows local installation

## Build

```powershell
npm --prefix plugins/accounts install
npm --prefix plugins/accounts run check
cargo test -p elegy-accountd -p elegy-accounts
cargo build --release -p elegy-accounts
cargo run -p elegy-tooling --bin elegy-plugin-packaging -- pack --plugin plugins/accounts --binary target/release/elegy-accounts.exe --binary-name bin/elegy-accounts.exe
```

## Brave extension and Native Messaging

1. Open `brave://extensions`, enable Developer mode, choose **Load unpacked**, and select `browser\brave` from the installed plugin.
2. Copy the extension ID shown by Brave.
3. Run `packaging\windows\install.ps1 -ExtensionId <32-character-id>`.
4. Reload the extension. On an origin declared by an installed provider pack, open Elegy Accounts and choose **Allow**.

Run the installed `start-account-center.ps1` to start the loopback-only UI, or `stop-account-center.ps1` to stop it. Provider packs are installed under `providers`; add a reviewed JSON pack there or set `ELEGY_ACCOUNTS_PROVIDER_DIR`, then restart the broker. Account Center renders the pack's authorization method and credential fields. Credentials, browser cookies, and global browser password stores are never imported.

The installer registers only the current user's Brave Native Messaging host. It does not request cookies or saved passwords. The generated host manifest allowlists the exact extension ID.

## Codex plugin

Install `elegy-accounts` from the generated local marketplace projection. The MCP server binary and provider packs are self-contained in the plugin and start on demand. After updating a development plugin, use the Codex plugin cachebuster/reinstall flow and test from a new task so the refreshed skill and MCP manifest load.

## Backup and uninstall

Vault backups are SQLite snapshots containing only metadata and DPAPI-protected AES-GCM envelopes. They are usable by the same Windows user on the same machine. Use `backup.ps1 -Destination <path>` and, while Account Center is stopped, `restore.ps1 -Source <path>`. Run `uninstall.ps1` to remove Native Messaging registration. The script intentionally retains encrypted data to prevent accidental account loss.
