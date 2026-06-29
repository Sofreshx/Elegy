---
name: elegy-desktop
description: High-risk desktop automation capabilities for bounded UI input and window management. Supports mouse clicks, text entry, key combinations, and strict window focus/move/minimize/maximize operations.
version: "2.0"
---

# Elegy Desktop Automation

High-risk desktop automation capabilities for bounded UI input and window management.

## Capabilities

- `desktop-click`: Simulate a mouse click at screen coordinates. Supports left, right, or middle button and dry-run preview.
- `desktop-type`: Inject Unicode text into the current foreground window.
- `desktop-key`: Send a parsed key combination such as ctrl+s, alt+tab, or enter.
- `desktop-focus`: Focus a single target window resolved by strict title matching or exact HWND.
- `desktop-move`: Move a target window and optionally resize it.
- `desktop-minimize`: Minimize a target window.
- `desktop-maximize`: Maximize a target window.
