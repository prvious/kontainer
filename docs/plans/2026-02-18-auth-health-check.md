# Auth Health Check Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace brittle client-side auth error detection with a server-validated `/api/me` health check that reliably logs users out on invalid tokens.

**Architecture:** A new `GET /api/me` endpoint behind `JWTMiddleware` returns the authenticated user from JWT claims. A new Tauri `check_auth` command calls it. A new `useAgentStatus` React hook polls it every 15s via TanStack Query, triggering `auth.logout()` on 401. This replaces the module-level `setAuthErrorHandler` hack, the string-based 401 detection in the query client, and the 30s client-side expiry timer.

**Tech Stack:** Go (Chi), Rust (Tauri + reqwest), React (TanStack Query), Vitest

---

### Task 1: Agent — `GET /api/me` handler

**Files:**
- Create: `agent/auth/me_handler.go`
- Modify: `agent/server/server.go:85-121` (add route inside JWT-protected group)

**Step 1: Write the Go test**

Create `agent/auth/me_handler_test.go`:

```go
package auth_test

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/prvious/kontainer/agent/auth"
	"github.com/golang-jwt/jwt/v5"
)

func TestMeHandler_ReturnsClaims(t *testing.T) {
	handler := auth.NewMeHandler()

	claims := &auth.Claims{
		RegisteredClaims: jwt.RegisteredClaims{Issuer: "kontainer-agent"},
		UserID:           "u1",
		Name:             "Jane Doe",
		Email:            "jane@example.com",
		AvatarURL:        "https://example.com/avatar.png",
		Provider:         "github",
	}

	req := httptest.NewRequest("GET", "/api/me", nil)
	ctx := context.WithValue(req.Context(), auth.ClaimsKey, claims)
	req = req.WithContext(ctx)

	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		User struct {
			ID        string `json:"id"`
			Name      string `json:"name"`
			Email     string `json:"email"`
			AvatarURL string `json:"avatarUrl"`
			Provider  string `json:"provider"`
		} `json:"user"`
		Status string `json:"status"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatalf("failed to decode response: %v", err)
	}
	if resp.User.ID != "u1" {
		t.Errorf("expected user ID u1, got %s", resp.User.ID)
	}
	if resp.User.Name != "Jane Doe" {
		t.Errorf("expected name Jane Doe, got %s", resp.User.Name)
	}
	if resp.Status != "ok" {
		t.Errorf("expected status ok, got %s", resp.Status)
	}
}

func TestMeHandler_NoClaims(t *testing.T) {
	handler := auth.NewMeHandler()

	req := httptest.NewRequest("GET", "/api/me", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401, got %d", w.Code)
	}
}
```

**Step 2: Run test to verify it fails**

Run: `go test -C /Users/clovismuneza/Apps/kontainer/agent ./auth/ -run TestMeHandler -v`
Expected: FAIL — `NewMeHandler` undefined

**Step 3: Write the handler**

Create `agent/auth/me_handler.go`:

```go
package auth

import (
	"encoding/json"
	"net/http"
)

type MeHandler struct{}

func NewMeHandler() *MeHandler {
	return &MeHandler{}
}

type meResponse struct {
	User   meUser `json:"user"`
	Status string `json:"status"`
}

type meUser struct {
	ID        string `json:"id"`
	Name      string `json:"name"`
	Email     string `json:"email"`
	AvatarURL string `json:"avatarUrl"`
	Provider  string `json:"provider"`
}

func (h *MeHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	claims, ok := r.Context().Value(ClaimsKey).(*Claims)
	if !ok || claims == nil {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(meResponse{
		User: meUser{
			ID:        claims.UserID,
			Name:      claims.Name,
			Email:     claims.Email,
			AvatarURL: claims.AvatarURL,
			Provider:  claims.Provider,
		},
		Status: "ok",
	})
}
```

**Step 4: Run test to verify it passes**

Run: `go test -C /Users/clovismuneza/Apps/kontainer/agent ./auth/ -run TestMeHandler -v`
Expected: PASS

**Step 5: Register the route**

In `agent/server/server.go`:
- Add `meHandler *auth.MeHandler` parameter to `New()` function (after `jwtMiddleware`)
- Add `r.Method("GET", "/me", meHandler)` inside the JWT-protected `r.Group` block (after the `r.Use(jwtMiddleware)` line, before the first existing route)

Also update `agent/main.go` where `server.New(...)` is called in `startNormalMode()`:
- Create `meHandler := auth.NewMeHandler()` before the `server.New()` call
- Pass `meHandler` as the new parameter

**Step 6: Run all Go tests**

Run: `go test -C /Users/clovismuneza/Apps/kontainer/agent ./...`
Expected: All packages PASS

**Step 7: Commit**

```bash
git add agent/auth/me_handler.go agent/auth/me_handler_test.go agent/server/server.go agent/main.go
git commit -m "feat: add GET /api/me endpoint for auth health check"
```

---

### Task 2: Tauri — `check_auth` command

**Files:**
- Create: `app/src-tauri/src/actions/agent/check_auth.rs`
- Modify: `app/src-tauri/src/actions/agent/mod.rs` (add `pub mod check_auth;`)
- Modify: `app/src-tauri/src/lib.rs` (register command in `invoke_handler!`)

**Step 1: Create the Rust command**

Create `app/src-tauri/src/actions/agent/check_auth.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub id: String,
    pub name: String,
    pub email: String,
    pub avatar_url: String,
    pub provider: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckAuthResponse {
    pub user: AuthUser,
    pub status: String,
}

#[tauri::command]
pub async fn check_auth(url: String, token: String) -> Result<CheckAuthResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/api/me"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if response.status().as_u16() == 401 {
        return Err("401".to_string());
    }

    if !response.status().is_success() {
        return Err(format!(
            "Agent responded with status {}",
            response.status()
        ));
    }

    let body: CheckAuthResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
```

**Key detail:** The 401 case returns `Err("401")` as a simple string — this is what the frontend hook will match on to distinguish auth failures from network errors.

**Step 2: Register the module and command**

In `app/src-tauri/src/actions/agent/mod.rs`, add alphabetically:
```rust
pub mod check_auth;
```

In `app/src-tauri/src/lib.rs`, add to `invoke_handler!` after the `ping_agent` line:
```rust
actions::agent::check_auth::check_auth,
```

**Step 3: Verify compilation**

Run: `cargo check --manifest-path /Users/clovismuneza/Apps/kontainer/app/src-tauri/Cargo.toml`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add app/src-tauri/src/actions/agent/check_auth.rs app/src-tauri/src/actions/agent/mod.rs app/src-tauri/src/lib.rs
git commit -m "feat: add check_auth Tauri command"
```

---

### Task 3: React — `useAgentStatus` hook

**Files:**
- Create: `app/src/hooks/use-agent-status.ts`
- Create: `app/tests/unit/use-agent-status.spec.tsx`

**Step 1: Write the test**

Create `app/tests/unit/use-agent-status.spec.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAgentStatus } from "@/hooks/use-agent-status";

// Provide a fresh QueryClient per test
function createWrapper() {
    const qc = new QueryClient({
        defaultOptions: { queries: { retry: false } },
    });
    return ({ children }: { children: React.ReactNode }) => (
        <QueryClientProvider client={qc}>{children}</QueryClientProvider>
    );
}

describe("useAgentStatus", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.useFakeTimers();
    });

    afterEach(() => {
        vi.useRealTimers();
    });

    it("calls check_auth with url and token", async () => {
        vi.mocked(invoke).mockResolvedValue({
            user: { id: "u1", name: "Jane", email: "j@e.com", avatarUrl: "", provider: "github" },
            status: "ok",
        });

        const onAuthFailure = vi.fn();
        renderHook(
            () => useAgentStatus("http://localhost:8080", "my-jwt", onAuthFailure),
            { wrapper: createWrapper() },
        );

        // Flush the initial query
        await act(async () => {
            vi.advanceTimersByTime(0);
        });

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("check_auth", {
                url: "http://localhost:8080",
                token: "my-jwt",
            });
        });

        expect(onAuthFailure).not.toHaveBeenCalled();
    });

    it("calls onAuthFailure on 401 error", async () => {
        vi.mocked(invoke).mockRejectedValue("401");

        const onAuthFailure = vi.fn();
        renderHook(
            () => useAgentStatus("http://localhost:8080", "expired-jwt", onAuthFailure),
            { wrapper: createWrapper() },
        );

        await act(async () => {
            vi.advanceTimersByTime(0);
        });

        await waitFor(() => {
            expect(onAuthFailure).toHaveBeenCalledTimes(1);
        });
    });

    it("does NOT call onAuthFailure on non-401 errors", async () => {
        vi.mocked(invoke).mockRejectedValue("Could not reach agent at http://localhost:8080: timeout");

        const onAuthFailure = vi.fn();
        renderHook(
            () => useAgentStatus("http://localhost:8080", "my-jwt", onAuthFailure),
            { wrapper: createWrapper() },
        );

        await act(async () => {
            vi.advanceTimersByTime(0);
        });

        // Give the query time to settle
        await act(async () => {
            vi.advanceTimersByTime(100);
        });

        expect(onAuthFailure).not.toHaveBeenCalled();
    });

    it("is disabled when url is null", async () => {
        const onAuthFailure = vi.fn();
        renderHook(
            () => useAgentStatus(null, "my-jwt", onAuthFailure),
            { wrapper: createWrapper() },
        );

        await act(async () => {
            vi.advanceTimersByTime(0);
        });

        expect(invoke).not.toHaveBeenCalledWith("check_auth", expect.anything());
    });

    it("is disabled when token is null", async () => {
        const onAuthFailure = vi.fn();
        renderHook(
            () => useAgentStatus("http://localhost:8080", null, onAuthFailure),
            { wrapper: createWrapper() },
        );

        await act(async () => {
            vi.advanceTimersByTime(0);
        });

        expect(invoke).not.toHaveBeenCalledWith("check_auth", expect.anything());
    });
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm --filter app test tests/unit/use-agent-status.spec.tsx`
Expected: FAIL — module `@/hooks/use-agent-status` not found

**Step 3: Write the hook**

Create `app/src/hooks/use-agent-status.ts`:

```ts
import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef } from "react";

const POLL_INTERVAL = 15_000;

export function useAgentStatus(
    agentUrl: string | null,
    token: string | null,
    onAuthFailure: () => void,
) {
    const onAuthFailureRef = useRef(onAuthFailure);
    onAuthFailureRef.current = onAuthFailure;

    const query = useQuery({
        queryKey: ["agent-status", agentUrl],
        queryFn: () => invoke("check_auth", { url: agentUrl, token }),
        enabled: !!agentUrl && !!token,
        refetchInterval: POLL_INTERVAL,
        retry: false,
    });

    useEffect(() => {
        if (query.error && String(query.error) === "401") {
            onAuthFailureRef.current();
        }
    }, [query.error]);

    return query;
}
```

**Design notes:**
- `onAuthFailureRef` avoids re-triggering the effect when the callback identity changes (logout is wrapped in `useCallback` but still re-creates).
- `retry: false` — no point retrying a 401.
- The `String(query.error) === "401"` check is exact (not `.includes()`), and only this one hook does it. All other queries use default retry behavior.

**Step 4: Run test to verify it passes**

Run: `pnpm --filter app test tests/unit/use-agent-status.spec.tsx`
Expected: PASS (all 5 tests)

**Step 5: Commit**

```bash
git add app/src/hooks/use-agent-status.ts app/tests/unit/use-agent-status.spec.tsx
git commit -m "feat: add useAgentStatus hook for server-side auth validation"
```

---

### Task 4: Mount hook in `_authenticated.tsx`

**Files:**
- Modify: `app/src/routes/_authenticated.tsx:1-43`

**Step 1: Wire up the hook**

In `app/src/routes/_authenticated.tsx`, add these imports at the top:

```ts
import { useAuth } from "@/hooks/use-auth";
import { useAgentStatus } from "@/hooks/use-agent-status";
import { useRouter } from "@tanstack/react-router";
```

Then update `AuthenticatedLayout` to call the hook:

```tsx
function AuthenticatedLayout() {
    const { agentUrl, token, logout } = useAuth();
    const router = useRouter();

    useAgentStatus(agentUrl, token, () => {
        logout().then(() =>
            router.navigate({ to: "/login", search: { redirect: undefined } }),
        );
    });

    return (
        <TerminalProvider>
            {/* ... rest unchanged ... */}
        </TerminalProvider>
    );
}
```

**Step 2: Verify compilation**

Run: `pnpm --filter app run types`
Expected: No type errors

**Step 3: Commit**

```bash
git add app/src/routes/_authenticated.tsx
git commit -m "feat: mount useAgentStatus in authenticated layout"
```

---

### Task 5: Cleanup — Remove `setAuthErrorHandler` hack

**Files:**
- Modify: `app/src/lib/query-client.ts` (simplify to plain QueryClient)
- Modify: `app/src/app.tsx` (remove setAuthErrorHandler useEffect and import)

**Step 1: Simplify query-client.ts**

Replace the entire contents of `app/src/lib/query-client.ts` with:

```ts
import { QueryClient } from "@tanstack/react-query";

export const queryClient = new QueryClient();
```

Everything removed:
- `QueryCache` import
- `AuthErrorHandler` type
- `onAuthError` module-level variable
- `setAuthErrorHandler` function
- `QueryCache.onError` handler
- `retry` function that string-matches 401

**Step 2: Clean up app.tsx**

In `app/src/app.tsx`:
- Remove the import: `import { setAuthErrorHandler } from "@/lib/query-client";`
- Remove the entire `useEffect` block that calls `setAuthErrorHandler` (lines 22-27)
- The `useAuth` import and the splashscreen `useEffect` and `useNativeBehavior` hook remain

The resulting `InnerApp` should be:

```tsx
function InnerApp() {
    const auth = useAuth();
    useNativeBehavior();

    useEffect(() => {
        if (!auth.isLoading) {
            invoke("close_splashscreen");
        }
    }, [auth.isLoading]);

    if (auth.isLoading) {
        return null;
    }

    return <RouterProvider router={router} context={{ auth }} />;
}
```

Also clean up unused imports — `useAuth` is still needed for `auth.isLoading` and passing context. Check if `setAuthErrorHandler` was the only reason for certain imports.

**Step 3: Run all frontend tests**

Run: `pnpm --filter app test`
Expected: All tests PASS. No test should reference `setAuthErrorHandler`.

**Step 4: Commit**

```bash
git add app/src/lib/query-client.ts app/src/app.tsx
git commit -m "refactor: remove setAuthErrorHandler hack from query client and app"
```

---

### Task 6: Cleanup — Remove client-side expiry timer

**Files:**
- Modify: `app/src/contexts/auth-context.tsx:161-195` (remove the 30s expiry timer `useEffect`)
- Modify: `app/tests/unit/token-expiry.spec.tsx` (rewrite tests)

**Step 1: Remove the expiry timer**

In `app/src/contexts/auth-context.tsx`, delete the entire `useEffect` block at lines 162-195 (the one with `setInterval` and `30_000`). This is the block that starts with:

```ts
// Expiry timer
useEffect(() => {
    const interval = setInterval(async () => {
```

Delete it entirely. The server-side `/api/me` check now handles expiry detection.

**Important:** Keep the session-restore `useEffect` (lines 108-159) intact — it still checks `expiresAt` on mount to avoid restoring an already-expired session. That's a one-time startup check, not a polling timer, and is still useful.

**Step 2: Update token-expiry tests**

The existing `token-expiry.spec.tsx` tests the 30s interval timer behavior. Since we're removing that timer, these tests need to be rewritten to test the new behavior: the mount-time expiry check in the session restore logic (which still exists).

Replace `app/tests/unit/token-expiry.spec.tsx` with:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { AuthProvider } from "@/contexts/auth-context";
import { useAuth } from "@/hooks/use-auth";
import type { StoredProfiles } from "@/types/connection";

function TestConsumer() {
    const { user, isLoading } = useAuth();
    return (
        <div>
            <span data-testid="user">{user ? user.name : "none"}</span>
            <span data-testid="isLoading">{isLoading ? "true" : "false"}</span>
        </div>
    );
}

describe("Token Expiry on Mount", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    function renderWithProvider() {
        return render(
            <AuthProvider>
                <TestConsumer />
            </AuthProvider>,
        );
    }

    it("does not restore a session with an expired token on mount", async () => {
        const nowSec = Math.floor(Date.now() / 1000);
        const expiredProfiles: StoredProfiles = {
            profiles: [
                {
                    id: "p1",
                    label: "Default",
                    agentUrl: "http://localhost:8080",
                    userId: "123",
                    userName: "Jane Doe",
                    userEmail: "jane@example.com",
                    userAvatarUrl: "https://example.com/avatar.png",
                    userProvider: "github",
                    token: "expired.jwt.token",
                    expiresAt: nowSec - 60, // expired 60 seconds ago
                },
            ],
            activeProfileId: "p1",
        };

        const emptyProfiles: StoredProfiles = {
            profiles: [],
            activeProfileId: null,
        };

        let deleted = false;
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return deleted ? emptyProfiles : expiredProfiles;
            if (cmd === "delete_profile") {
                deleted = true;
                return undefined;
            }
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        expect(screen.getByTestId("user")).toHaveTextContent("none");
        expect(invoke).toHaveBeenCalledWith("delete_profile", { profileId: "p1" });
    });

    it("restores a session with a valid token on mount", async () => {
        const nowSec = Math.floor(Date.now() / 1000);
        const validProfiles: StoredProfiles = {
            profiles: [
                {
                    id: "p1",
                    label: "Default",
                    agentUrl: "http://localhost:8080",
                    userId: "123",
                    userName: "Jane Doe",
                    userEmail: "jane@example.com",
                    userAvatarUrl: "https://example.com/avatar.png",
                    userProvider: "github",
                    token: "valid.jwt.token",
                    expiresAt: nowSec + 99999,
                },
            ],
            activeProfileId: "p1",
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return validProfiles;
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        expect(screen.getByTestId("user")).toHaveTextContent("Jane Doe");
    });
});
```

**Step 3: Run the updated tests**

Run: `pnpm --filter app test tests/unit/token-expiry.spec.tsx`
Expected: PASS (2 tests)

**Step 4: Run all frontend tests**

Run: `pnpm --filter app test`
Expected: All tests PASS

**Step 5: Commit**

```bash
git add app/src/contexts/auth-context.tsx app/tests/unit/token-expiry.spec.tsx
git commit -m "refactor: remove 30s client-side expiry timer, replaced by server-side health check"
```

---

### Task 7: Quality checks

**Step 1: Run Go tests**

Run: `go test -C /Users/clovismuneza/Apps/kontainer/agent ./...`
Expected: All packages PASS

**Step 2: Run Rust check**

Run: `cargo check --manifest-path /Users/clovismuneza/Apps/kontainer/app/src-tauri/Cargo.toml`
Expected: Compiles cleanly

**Step 3: Run TypeScript type check**

Run: `pnpm --filter app run types`
Expected: No errors

**Step 4: Run frontend tests**

Run: `pnpm --filter app test`
Expected: All tests PASS

**Step 5: Run lint**

Run: `pnpm run lint`
Expected: Clean (possibly one pre-existing warning about fast-refresh)

**Step 6: Run formatter**

Run: `pnpm fmt`
Expected: Files formatted

**Step 7: Commit formatting if needed**

```bash
git add -A
git commit -m "chore: format auth health check files"
```
