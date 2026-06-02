# Install and Enable the Official Obsidian CLI

The `elegy-obsidian` skill shells out to the **official** `obsidian` CLI that ships with Obsidian Desktop 1.12 and later. There is no separate binary to install from Elegy. This file walks through verifying and enabling it.

## Requirements

- Obsidian Desktop 1.12 or later (any edition: Catalyst, Standard, or Commercial).
- The `obsidian` binary on `PATH` after enabling the CLI. Obsidian installs the binary into a per-OS location and then offers to add it to `PATH` from the toggle switch.

## Enable the CLI (one time)

1. Open Obsidian Desktop.
2. Go to **Settings -> General -> Command line interface**.
3. Toggle **"Install Obsidian command line tool"** (or **"Enable Obsidian command line interface"** depending on build) to on.
4. Obsidian will prompt to add `obsidian` to your system `PATH`. Accept the prompt.
5. Quit and reopen Obsidian once to make sure the binary is registered.

## Verify

Open a new terminal session (so the updated `PATH` is loaded) and run:

```bash
obsidian version
```

You should see a line like `1.12.x`. If the command is not found:

- macOS / Linux: ensure `~/.local/bin` (or the path Obsidian surfaced) is in your shell `PATH`.
- Windows: ensure `%LOCALAPPDATA%\Programs\Obsidian` is on `PATH` for your user, or run the suggested "Add to PATH" toggle again and reopen the terminal.

## Sanity check the skill

From the repo root, after enabling the CLI:

```bash
obsidian vault
obsidian read file=README.md
```

If both succeed, the `elegy-obsidian` skill is ready to use.

## When the CLI is unavailable

Some environments cannot install Obsidian Desktop (for example, headless CI, locked-down sandboxes, or server-only containers). In those cases the `elegy-obsidian` skill will not work because the official CLI is part of the desktop application. Do **not** fall back to a custom binary: the skill's authority boundary is "wraps the official CLI", and a parallel binary would silently diverge from Obsidian's behavior. Instead, report the gap and route the user to a host with Obsidian Desktop installed.

## Upgrades

Obsidian releases new CLI commands with each minor version. After upgrading Obsidian Desktop, re-run `obsidian version` and review the `obsidian-cli-command-reference.md` to learn which capabilities gained new modes (e.g. new `format=json` outputs, new `task state` values, or new `patch` modes).
