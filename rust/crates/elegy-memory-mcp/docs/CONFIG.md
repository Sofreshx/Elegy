# elegy-memory-mcp configuration

WU2 adds the startup configuration surface for `elegy-memory-mcp`.

## Required environment variables

The binary refuses to start and exits non-zero when any required variable is missing or empty.

| Variable | Purpose |
|---|---|
| `ELEGY_MCP_ADMIN_PASSWORD` | Admin password checked by the OAuth consent flow. Provide the password itself; startup derives an Argon2 verifier in memory and rejects pre-hashed Argon2 strings from the old WU4 docs. |
| `ELEGY_MCP_DB_PATH` | Path to the SQLite database used by `elegy-memory`. |
| `ELEGY_MCP_PUBLIC_URL` | Public tunnel URL used for future OAuth metadata and auth headers. |

## Optional environment variables

| Variable | Default | Notes |
|---|---|---|
| `ELEGY_MCP_PORT` | `8765` | Must be an integer from `1` to `65535`. |
| `ELEGY_MCP_LOG_CONTENT` | `0` | Boolean parser accepts `0/1`, `true/false`, `yes/no`, `on/off`. |
| `ELEGY_MCP_DATA_DIR` | `directories::ProjectDirs::from("com", "holon", "elegy-mcp").data_local_dir()` | On Windows this resolves under `%LOCALAPPDATA%\\holon\\elegy-mcp`. |

## Logging

- Startup initializes `tracing-subscriber` JSON logs on stdout.
- WU2 only logs safe startup metadata (`port`, `public_url`, `db_path`, `data_dir`, `log_content`).
- Passwords, derived verifiers, tokens, codes, signing keys, and memory content are not logged.

## Scope note

WU4 adds OAuth endpoint wiring, but `/mcp` remains intentionally public until WU5 bearer validation middleware lands.
