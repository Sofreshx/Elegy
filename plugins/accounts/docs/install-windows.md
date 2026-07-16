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
4. Reload the extension. On a supported provider page, open Elegy Accounts and choose **Allow**.

Run the installed `start-account-center.ps1` to start the loopback-only UI, or `stop-account-center.ps1` to stop it. GitHub uses its device authorization flow. Cloudflare guides the user to create a resource-limited API token and validates it before storage; passwords, browser cookies, and global API keys are not imported.

The installer registers only the current user's Brave Native Messaging host. It does not request cookies or saved passwords. The generated host manifest allowlists the exact extension ID.

## Codex plugin

Install `elegy-accounts` from the generated Elegy marketplace projection. The MCP server binary is self-contained under the plugin `bin` directory and starts on demand.

## Backup and uninstall

Vault backups are SQLite snapshots containing only metadata and DPAPI-protected AES-GCM envelopes. They are usable by the same Windows user on the same machine. Use `backup.ps1 -Destination <path>` and, while Account Center is stopped, `restore.ps1 -Source <path>`. Run `uninstall.ps1` to remove Native Messaging registration. The script intentionally retains encrypted data to prevent accidental account loss.
