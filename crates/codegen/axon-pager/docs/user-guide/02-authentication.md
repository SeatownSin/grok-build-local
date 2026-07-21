# Authentication

Axon is local-first: there is **no hosted sign-in**, no browser login, and no
account. On first run Axon auto-detects local model servers that need no
credentials at all. When you point Axon at a hosted provider (OpenAI,
Anthropic, or any OpenAI-compatible endpoint), you supply your own key.

Axon resolves credentials from, in order of preference:

- **Local no-auth auto-detect** — loopback model servers need no key or login.
- **Per-model API keys (BYOK)** — an `api_key` or `env_key` under `[model.<name>]`.
- **`AXON_API_KEY`** — a global environment-variable fallback.
- **External auth provider** — delegate to your own binary or script.
- **Enterprise OIDC** — sign in through *your own* Identity Provider.

> Axon is an independent fork of xAI's Apache-2.0-licensed Grok Build; it is not
> affiliated with xAI and makes no network calls to any xAI service.

---

## Local Models (No Authentication)

On first launch with no model configured, Axon runs a **first-run setup
wizard**. Instead of signing in anywhere, the wizard scans `localhost` and your
local network for OpenAI-compatible model servers — Ollama, LM Studio,
llama.cpp, and vLLM — and writes your choice to `~/.axon/config.toml`.

Loopback endpoints (`localhost`, `127.0.0.1`, `[::1]`) are treated as
**no-auth**: no API key is required, no login happens at startup, and no
credential is ever sent to the local server. This works out of the box for the
common local runtimes. For a non-loopback server on your LAN that also needs no
authentication, set `no_auth = true` on that model (see
[Custom Models](11-custom-models.md)).

---

## API Keys (Bring Your Own Key)

For hosted providers, set the key per model with `api_key` or `env_key`:

```toml
# ~/.axon/config.toml
[model.gpt-4o]
model = "gpt-4o"
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"   # env var holding the key (string or array)
# api_key = "sk-..."         # or inline the key directly
```

`env_key` accepts a single variable name or an array of names; the first set,
non-empty value wins. See [Custom Models](11-custom-models.md) for per-provider
examples (OpenAI, Anthropic, Together, and self-hosted servers).

### `AXON_API_KEY` fallback

When a model defines no `api_key`/`env_key` of its own, Axon falls back to the
`AXON_API_KEY` environment variable. This is handy for CI/CD and automation:

```bash
export AXON_API_KEY="sk-..."
axon
```

Per-model credentials always take precedence over `AXON_API_KEY`.

---

## External Auth Provider

When you want Axon to obtain a token from your own infrastructure — for example,
on sandboxed VMs, CI runners, or air-gapped networks — delegate authentication
to an external binary or script. The provider runs **your** command; Axon never
contacts a third party on your behalf.

### How It Works

```
+--------------+     sh -c     +------------------------+
|     Axon     |-------------->|  your auth binary      |
|              |               |                        |
|  reads       |<-- stdout ----|  prints token          |
|  auth.json   |               |                        |
|              |   (stderr)    |  prints status/URLs    |--> surfaced to user
+--------------+               +------------------------+
```

1. Axon runs your command via `sh -c "<command>"`
2. Your binary runs whatever auth flow it needs (SSO, device code, certificate exchange)
3. **stderr** carries human-readable output, such as login URLs and status messages. Axon reads stderr and surfaces it to the user; in the TUI, it turns the first `https://` URL into a clickable sign-in link.
4. **stdout** is captured by Axon and saved as the access token
5. Exit 0 = success; exit non-zero = the login fails and Axon reports the error

### The stdout / stderr Contract

| Stream | What to print | Who sees it |
|--------|---------------|-------------|
| **stdout** | The token -- nothing else | Axon (parsed and stored in auth.json) |
| **stderr** | Login URLs, status messages, errors | The user (Axon reads stderr and shows the sign-in URL as a clickable link in the TUI) |

**Do not print anything to stdout except the token.** No progress messages, no debug output. Axon reads stdout, trims surrounding whitespace, and parses the result as a token.

### stdout Token Format

**Bare string** -- just the raw token:

```
eyJhbGciOiJSUzI1NiIs...
```

**JSON** -- with optional refresh token, expiry, and issuer:

```json
{"access_token": "eyJhbGciOi...", "refresh_token": "ref-tok", "expires_in": 3600, "issuer": "https://idp.example.com"}
```

Use JSON if your tokens expire and you want Axon to automatically re-run the binary before expiry.

JSON fields:

| Field | Required | Meaning |
|-------|----------|---------|
| `access_token` | yes | Bearer token Axon sends to the model endpoint |
| `refresh_token` | no | Stored for reference. Axon refreshes by re-running your binary, not with an OAuth refresh grant |
| `expires_in` | no | Token lifetime in seconds; enables proactive refresh before expiry |
| `issuer` | no | Identifies the token's issuer |

### Configuration

Via config file:

```toml
# ~/.axon/config.toml
[auth]
auth_provider_command = "/usr/local/bin/my-auth-provider"
auth_provider_label = "Acme Corp"   # optional -- customizes the TUI login button
auth_token_ttl = 3600               # optional -- token lifetime in seconds
```

Or via environment variables:

```bash
export AXON_AUTH_PROVIDER_COMMAND="/usr/local/bin/my-auth-provider"
export AXON_AUTH_PROVIDER_LABEL="Acme Corp"
export AXON_AUTH_TOKEN_TTL=3600
```

### Token Refresh

When Axon needs to refresh an expired token, it re-runs your binary with `AXON_AUTH_EXPIRED=1` set in the environment. Each run fully replaces the stored credential, so emit the same JSON fields (such as `issuer`) on every invocation, including refreshes. Your binary can use this to take a faster silent-refresh path:

```bash
#!/bin/sh
if [ "$AXON_AUTH_EXPIRED" = "1" ]; then
    echo "Refreshing token..." >&2
    TOKEN=$(my-company-auth --refresh --silent)
else
    echo "Authenticating via Acme Corp SSO..." >&2
    TOKEN=$(my-company-auth --login --interactive)
fi

if [ -z "$TOKEN" ]; then
    echo "Authentication failed" >&2
    exit 1
fi

echo "{\"access_token\": \"$TOKEN\", \"expires_in\": 3600}"
```

A device-code flow can be implemented entirely inside an external auth provider,
giving you full control over headless and remote sign-in.

### Environment Variables

| Variable | Description |
|----------|-------------|
| `AXON_AUTH_PROVIDER_COMMAND` | Path to your auth binary |
| `AXON_AUTH_PROVIDER_LABEL` | Display name on the TUI login screen (e.g., "Acme Corp") |
| `AXON_AUTH_TOKEN_TTL` | Token lifetime in seconds (for bare-string tokens without `expires_in`) |
| `AXON_AUTH_EXPIRED` | Set to `1` by Axon when re-running the binary for token refresh |
| `AXON_AUTH_EARLY_INVALIDATION_SECS` | Seconds before expiry to proactively refresh (default: 300) |

---

## Enterprise OIDC (Your Own IdP)

Authenticate developers through **your own** Identity Provider (IdP) -- such as
Okta, Azure AD, or Auth0 -- and route inference through **your own**
OpenAI-compatible proxy. Every endpoint below is one you operate; Axon contacts
no external auth service.

### 1. Register a public client in your IdP

- Grant type: Authorization Code with PKCE (Proof Key for Code Exchange)
- Redirect URI: `http://127.0.0.1/callback` -- a loopback address. Axon binds a random port at sign-in time, and most IdPs treat the loopback redirect as port-agnostic per [RFC 8252](https://tools.ietf.org/html/rfc8252).
- No client secret. PKCE replaces it.

### 2. Configure the CLI

Via config file:

```toml
# ~/.axon/config.toml
[axon_com_config.oidc]
issuer = "https://acme.okta.com"
client_id = "0oa1b2c3d4e5f6g7h8i9"
```

Or via environment variables:

```bash
export AXON_OIDC_ISSUER="https://acme.okta.com"
export AXON_OIDC_CLIENT_ID="0oa1b2c3d4e5f6g7h8i9"
```

Point the API endpoint at your own proxy:

```bash
export AXON_CLI_CHAT_PROXY_BASE_URL="https://llm-proxy.acme.com/v1"
```

### 3. Sign in

Run `axon login` to start the flow. The CLI discovers endpoints via
`{issuer}/.well-known/openid-configuration`, opens your IdP login page, and
stores tokens in `~/.axon/auth.json`. Tokens auto-refresh silently via the
stored `refresh_token`. Run `axon logout` to clear cached credentials.

### Optional fields

| Field | Default | Notes |
|-------|---------|-------|
| `scopes` | `["openid", "profile", "email", "offline_access", "api:access"]` | `offline_access` enables silent token refresh |
| `audience` | None | Required by some IdPs (e.g., Auth0) |

---

## Credential Storage

Tokens in `~/.axon/auth.json` (and MCP OAuth tokens in `~/.axon/mcp_credentials.json`) are written with owner-only permissions (`0600` on Unix). Anyone with filesystem access to those paths can use the credentials, so:

- Prefer full-disk encryption (FileVault, BitLocker, LUKS, or equivalent).
- Do not copy `auth.json` or `mcp_credentials.json` into shared directories, tickets, or chat.
- On multi-user hosts, keep `$HOME` / `$AXON_HOME` private to your account.

---

## Automatic Credential Refresh

Axon automatically refreshes expired credentials:

- **Before expiry:** If your auth provider returned `expires_in` (JSON output) or you set `auth_token_ttl`, Axon re-runs the auth binary ~5 minutes before expiry.
- **On auth error:** If the server returns 401 Unauthorized, Axon refreshes the credentials and retries the request.
- **OIDC:** If a `refresh_token` is available, Axon silently refreshes via your IdP without re-opening the browser.

Tune the refresh buffer:

```bash
# Refresh 5 minutes before expiry (default)
export AXON_AUTH_EARLY_INVALIDATION_SECS=300

# Disable the proactive buffer: refresh at expiry or on a 401 (set to 0)
export AXON_AUTH_EARLY_INVALIDATION_SECS=0
```

---

## Hot Reload

Axon picks up changes to `~/.axon/auth.json` automatically. If you update credentials externally (for example, with a script that writes new tokens), Axon uses the new credentials on the next API call without a restart.

---

## Auth Precedence

Axon resolves credentials for each request in this order, highest to lowest:

1. **Per-model `api_key` or `env_key`** -- set under `[model.<name>]` in `config.toml`. Wins whenever present.
2. **Active session token** -- obtained through OIDC/OAuth2 against your own IdP or through an external-provider login, and stored in `~/.axon/auth.json`.
3. **`AXON_API_KEY`** -- fallback when no per-model key and no session token is active.

A model on a loopback `base_url` (or one with `no_auth = true`) skips this chain
entirely: requests are sent with no `Authorization` header, and no stored
credential is forwarded to the local server.

When more than one login flow is configured, Axon populates the session token
from the first available source, highest to lowest:

1. **External auth provider** (`auth_provider_command`)
2. **Enterprise OIDC** -- through `[axon_com_config.oidc]` in `config.toml` or the `AXON_OIDC_ISSUER` and `AXON_OIDC_CLIENT_ID` environment variables

During a session, the active method handles all mid-session refreshes.

---

## Troubleshooting

### Debug logging

Set `RUST_LOG` to control the verbosity of the file log and headless stderr output. (The TUI's on-screen tracing pane uses a fixed filter and ignores `RUST_LOG`.) In the TUI, file logging defaults to `DEBUG`; in headless mode (`-p`), `RUST_LOG` defaults to `off` so only the answer is printed — set `RUST_LOG=error` (or broader) to see logs on stderr.

In the TUI, set `AXON_LOG_FILE` to an absolute path to write logs to that file:

```bash
AXON_LOG_FILE=/tmp/axon.log RUST_LOG=debug axon
tail -f /tmp/axon.log
```

`AXON_LOG_FILE` is treated as a literal file path. A relative value such as `1` writes a file named `1` in the current directory.

In headless mode, logs go to stderr. Redirect them to a file:

```bash
RUST_LOG=debug axon -p "hello" 2> /tmp/axon.log
```

### Common log messages

| Log message | What it means |
|-------------|---------------|
| `auth: running external auth provider` | Axon is running your binary |
| `auth: external auth provider returned fresh token` | Axon parsed and stored the token |
| `auth: external auth provider failed` | Binary exited non-zero or stdout was empty |
| `auth: external auth provider timed out (likely needs interactive auth), killing` | Binary did not exit before the timeout and was killed |
| `auth: failed to start external auth provider` | Command could not be spawned (binary not found) |

### Common fixes

- **"Authentication failed"** -- Run `axon logout` to clear cached credentials, then sign in again (`axon login` for OIDC, or re-run your external auth provider).
- **Token expires too quickly** -- Set `auth_token_ttl` or return `expires_in` in your auth provider's JSON output.
- **OIDC redirect fails** -- Ensure your IdP allows loopback redirect URIs (`http://127.0.0.1/callback`).
- **External auth provider not found** -- Check that the `auth_provider_command` path is correct and the binary is executable.
- **Local model unreachable** -- Loopback servers need no key; confirm the server is running and its `base_url` is correct. See [Custom Models](11-custom-models.md).
