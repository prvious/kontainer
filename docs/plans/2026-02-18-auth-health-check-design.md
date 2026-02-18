# Auth Health Check Design

## Problem

The current auth error handling is fragile and incomplete:

1. **String-based 401 detection**: `String(error).includes("401")` in `query-client.ts`
2. **403 completely unhandled**: Agent returns 403 but app ignores it
3. **Module-level hack**: `setAuthErrorHandler` bridges React context to query client via global callback
4. **Client-only expiry timer**: 30s interval checks `expiresAt` locally, never validates server-side

## Solution

Replace all client-side guesswork with a single server-validated health check.

### Agent: `GET /api/me`

New endpoint behind existing `JWTMiddleware`. Returns authenticated user info from JWT claims:

```json
{
  "user": { "id": "...", "name": "...", "email": "...", "avatar_url": "...", "provider": "github" },
  "status": "ok"
}
```

On invalid/expired token, `JWTMiddleware` returns 401 automatically.

### Tauri: `check_auth` command

New Rust command: `invoke("check_auth", { url, token })`. Returns the `/api/me` response on success, `Err("401")` on auth failure, generic error string otherwise.

### React: `useAgentStatus` hook

TanStack Query hook polling `check_auth` every 15 seconds:

- **Enabled** only when `agentUrl` and `token` are present
- **On 401**: calls `auth.logout()` — route guard redirects to `/login`
- **On network/other errors**: ignored (transient failures shouldn't log out)
- **Mounted in** `_authenticated.tsx` layout (single instance)

### What gets deleted

1. `app/src/app.tsx` — `useEffect` calling `setAuthErrorHandler`
2. `app/src/lib/query-client.ts` — `setAuthErrorHandler`, `onAuthError`, `QueryCache.onError` 401 matching, retry 401 check. Becomes plain `new QueryClient()`
3. `app/src/contexts/auth-context.tsx` — 30s client-side expiry timer

### Error discrimination

The `useAgentStatus` hook is the **single place** that handles auth failures. Other hooks let errors bubble up normally for error UI. No more string matching across multiple code paths.

### Setup mode edge case

If a connected agent re-enters setup mode, `/api/me` gets 403 (setup mode rejects `/api/*`). The hook treats non-401 errors as transient. User must reconnect through login screen where `ping_agent` detects `status: "setup_required"`.
