# Auth Flow Redesign — Plan 2 of 5

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the JWT interconnection gap between Goravel and the Tauri layer, then redesign the auth flow screens (splash, connect, authenticating, cluster switcher) to match the new design in `docs/design.pen`.

**Architecture:** The Goravel backend generates standard JWT tokens with only `sub` (user ID) and `exp` claims — no user profile data. The current Rust parser expects custom claims with full user info. Fix: simplify the Rust JWT parser to only extract `sub` + `exp`, then use the existing `check_auth` command (which calls `/api/me`) to fetch user details. Also fix the user ID type mismatch (Go returns `uint`, Rust expects `string`). Then redesign all auth UI screens.

**Tech Stack:** Go (Goravel), Rust (Tauri), React, TanStack Router, Tailwind CSS v4, Lucide icons

**Design reference:** `docs/design.pen` — screens `PwbGj` (Splash), `WDz3B` (Connect), `hO63m` (Authenticating), `syxFU` (Cluster Switcher)

---

## Interconnection Gaps Identified

| Gap | Current State | Fix |
|-----|--------------|-----|
| **JWT payload** | Rust `JwtPayload` expects `{user_id, name, email, avatar_url, provider, exp}` | Goravel JWT has `{key, sub, exp}` where `key` = user ID and `sub` = guard name ("user"). Change Rust to extract `key` + `exp`, fetch user from `/api/me` |
| **User ID type** | Go `MeController` returns `user.ID` as `uint` (JSON number) | Rust `AuthUser.id` expects `String`. Fix: cast to string in Go response |
| **ParsedToken** | TS `ParsedToken` has `{ user: AuthUser, expiresAt }` | New: `{ userId: string, expiresAt: number }` — user info comes from `check_auth` |

---

## File Structure

### Modified files
- `server/app/http/controllers/auth/me_controller.go` — Return user ID as string
- `app/src-tauri/src/actions/auth/mod.rs` — Simplify `JwtPayload` and `ParsedToken` structs
- `app/src-tauri/src/actions/auth/parse_auth_token.rs` — Update to use `sub` claim
- `app/src/types/auth.ts` — Update `ParsedToken` type
- `app/src/contexts/auth-context.tsx` — Use `check_auth` for user info after token
- `app/src/components/login-screen.tsx` — Full redesign to match Connect screen
- `app/src/app.tsx` — React splash screen instead of native Tauri splash
- `app/src/routes/login.tsx` — Minor layout update
- `app/tests/unit/login-screen.spec.tsx` — Update test expectations
- `app/tests/unit/auth-context.spec.tsx` — Update for new handleToken flow

### New files
- `app/src/components/splash-screen.tsx` — Splash screen component
- `app/src/components/authenticating-screen.tsx` — OAuth-in-progress transition screen

---

### Task 1: Fix Go MeController — Return User ID as String

**Files:**
- Modify: `server/app/http/controllers/auth/me_controller.go:26`

The Goravel ORM `User.ID` is `uint`. The Rust `AuthUser.id` expects a `String`. When Go serializes `"id": user.ID`, it produces a JSON number (`1`), which fails Rust's `serde` string deserialization.

- [ ] **Step 1: Import fmt and cast the ID**

In `server/app/http/controllers/auth/me_controller.go`, add `"fmt"` to the imports and change line 26:

```go
"id": fmt.Sprintf("%d", user.ID),
```

The full response block becomes:
```go
return ctx.Response().Json(nethttp.StatusOK, map[string]any{
    "user": map[string]any{
        "id":        fmt.Sprintf("%d", user.ID),
        "name":      user.Name,
        "email":     user.Email,
        "avatarUrl": user.AvatarURL,
        "provider":  user.Provider,
    },
    "status": "ok",
})
```

- [ ] **Step 2: Run Go vet**

```bash
go vet -C /Users/clovismuneza/Apps/kontainer/server ./...
```

- [ ] **Step 3: Run Go tests**

```bash
go test -C /Users/clovismuneza/Apps/kontainer/server ./...
```

- [ ] **Step 4: Commit**

```bash
git add server/app/http/controllers/auth/me_controller.go
git commit -m "fix: return user ID as string in /api/me response for Rust compatibility"
```

---

### Task 2: Fix Rust JWT Parser for Goravel JWT Format

**Files:**
- Modify: `app/src-tauri/src/actions/auth/mod.rs`
- Modify: `app/src-tauri/src/actions/auth/parse_auth_token.rs`

Goravel's JWT payload uses `{"key": "1", "sub": "user", "exp": 1740000000, "iat": ...}` where `key` is the user ID and `sub` is the guard name (always "user"). The current Rust code expects custom claims like `user_id`, `name`, `email`, etc.

- [ ] **Step 1: Simplify JwtPayload in mod.rs**

Replace the `JwtPayload` struct to match Goravel's format:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct JwtPayload {
    pub key: String,
    pub exp: u64,
}
```

Replace the `ParsedToken` struct (remove `user` field, expose `key` as `userId`):

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParsedToken {
    pub user_id: String,
    pub expires_at: u64,
}
```

Remove the `AuthUser` struct from `mod.rs` (it still exists in `actions/agent/check_auth.rs` which is where it's actually needed).

- [ ] **Step 2: Update parse_auth_token.rs**

```rust
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

use super::{JwtPayload, ParsedToken};

#[tauri::command]
pub fn parse_auth_token(token: &str) -> Result<ParsedToken, String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err("invalid token format".into());
    }

    let payload_bytes = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|e| format!("base64 decode error: {e}"))?;

    let payload: JwtPayload =
        serde_json::from_slice(&payload_bytes).map_err(|e| format!("json parse error: {e}"))?;

    Ok(ParsedToken {
        user_id: payload.key,
        expires_at: payload.exp,
    })
}
```

- [ ] **Step 3: Verify the `AuthUser` struct in check_auth.rs is self-contained**

Read `app/src-tauri/src/actions/agent/check_auth.rs` — it already has its own `AuthUser` struct. Verify it compiles without the one we removed from `mod.rs`. If any other file imported `AuthUser` from `actions::auth`, update that import to point at `actions::agent::check_auth`.

- [ ] **Step 4: Build and verify**

```bash
cargo check --manifest-path /Users/clovismuneza/Apps/kontainer/app/src-tauri/Cargo.toml
```

Fix any compilation errors from removed/renamed types.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/src/actions/auth/mod.rs app/src-tauri/src/actions/auth/parse_auth_token.rs
git commit -m "fix: simplify JWT parser for Goravel standard claims (sub + exp)"
```

---

### Task 3: Update React Auth Context for New JWT Flow

**Files:**
- Modify: `app/src/types/auth.ts`
- Modify: `app/src/contexts/auth-context.tsx`

After getting the JWT from the WebSocket bridge, we now need two steps:
1. `parse_auth_token(token)` → get `{ sub, expiresAt }` (just the user ID and expiry)
2. `check_auth(url, token)` → get `{ user: { id, name, email, avatarUrl, provider }, status }` (full user info from `/api/me`)

- [ ] **Step 1: Update ParsedToken in types/auth.ts**

```typescript
export interface ParsedToken {
    userId: string;
    expiresAt: number;
}
```

Remove the `user: AuthUser` field. Keep `AuthUser`, `AuthState`, and `StoredSession` unchanged.

Add the `CheckAuthResponse` type:

```typescript
export interface CheckAuthResponse {
    user: AuthUser;
    status: string;
}
```

- [ ] **Step 2: Update handleToken in auth-context.tsx**

Replace the `handleToken` callback:

```typescript
const handleToken = useCallback(async (jwt: string, loginAgentUrl: string) => {
    try {
        // Step 1: Get expiry from JWT claims
        const parsed = await invoke<ParsedToken>("parse_auth_token", { token: jwt });

        // Step 2: Get user info from agent's /api/me
        const authResponse = await invoke<CheckAuthResponse>("check_auth", {
            url: loginAgentUrl,
            token: jwt,
        });

        const profileId = crypto.randomUUID();
        const label = pendingLabelRef.current;

        setUser(authResponse.user);
        setToken(jwt);
        setExpiresAt(parsed.expiresAt);
        setAgentUrl(loginAgentUrl);

        try {
            await invoke("save_profile", {
                input: {
                    id: profileId,
                    label,
                    agentUrl: loginAgentUrl,
                    userId: authResponse.user.id,
                    userName: authResponse.user.name,
                    userEmail: authResponse.user.email,
                    userAvatarUrl: authResponse.user.avatarUrl,
                    userProvider: authResponse.user.provider,
                    token: jwt,
                    expiresAt: parsed.expiresAt,
                },
            });
        } catch (err) {
            console.error("Failed to save profile:", err);
        }

        // Reload profiles
        try {
            const data = await invoke<StoredProfiles>("load_profiles");
            setProfiles(data.profiles);
            setActiveProfileId(data.activeProfileId);
        } catch {
            setActiveProfileId(profileId);
        }
    } catch (err) {
        console.error("Failed to handle auth token:", err);
    }
}, []);
```

Add the import for `CheckAuthResponse`:
```typescript
import type { AuthUser, ParsedToken, CheckAuthResponse } from "@/types/auth";
```

- [ ] **Step 3: Verify types pass**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 4: Commit**

```bash
git add app/src/types/auth.ts app/src/contexts/auth-context.tsx
git commit -m "feat: use check_auth for user info, parse_auth_token for expiry only"
```

---

### Task 4: Create React Splash Screen

**Files:**
- Create: `app/src/components/splash-screen.tsx`
- Modify: `app/src/app.tsx`

The design shows a centered splash with: accent gradient top bar, container logo icon in a gradient border box, "kontainer" brand text, "Infrastructure, Simplified" subtitle, animated progress bar, "Initializing runtime..." text, and version number.

- [ ] **Step 1: Create splash-screen.tsx**

```tsx
import { Container } from "lucide-react";

export function SplashScreen() {
    return (
        <div className='flex h-screen flex-col bg-background'>
            {/* Accent line */}
            <div
                className='h-0.5 w-full shrink-0'
                style={{ background: "linear-gradient(90deg, #00FF88, #00D4FF 50%, #A78BFA)" }}
            />

            <div className='flex flex-1 items-center justify-center'>
                <div className='flex w-[400px] flex-col items-center gap-7'>
                    {/* Logo */}
                    <div className='flex size-[88px] items-center justify-center rounded-[20px] border-[1.5px] border-accent-green/40 bg-gradient-to-b from-accent-green/15 to-accent-cyan/8'>
                        <Container className='size-11 text-accent-green' />
                    </div>

                    {/* Brand */}
                    <div className='flex flex-col items-center gap-2.5'>
                        <span className='text-[38px] font-bold text-foreground'>kontainer</span>
                        <span className='font-mono text-sm font-medium text-text-secondary'>Infrastructure, Simplified</span>
                    </div>

                    {/* Loading */}
                    <div className='flex w-[280px] flex-col items-center gap-3'>
                        <div className='h-[3px] w-full overflow-hidden rounded-sm bg-bg-subtle'>
                            <div className='h-full w-1/3 animate-pulse rounded-sm bg-gradient-to-r from-accent-green to-accent-cyan' />
                        </div>
                        <span className='font-mono text-[11px] font-medium text-muted-foreground'>Initializing runtime...</span>
                    </div>

                    {/* Version */}
                    <span className='font-mono text-[10px] font-medium text-muted-foreground'>v1.0.0</span>
                </div>
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Update app.tsx to show SplashScreen during loading**

Replace the `InnerApp` component:

```tsx
import { SplashScreen } from "@/components/splash-screen";

function InnerApp() {
    const auth = useAuth();
    useNativeBehavior();

    useEffect(() => {
        if (!auth.isLoading) {
            invoke("close_splashscreen");
        }
    }, [auth.isLoading]);

    if (auth.isLoading) {
        return <SplashScreen />;
    }

    return <RouterProvider router={router} context={{ auth }} />;
}
```

- [ ] **Step 3: Verify types pass**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 4: Commit**

```bash
git add app/src/components/splash-screen.tsx app/src/app.tsx
git commit -m "feat: add React splash screen with logo, progress bar, and brand"
```

---

### Task 5: Create Authenticating Transition Screen

**Files:**
- Create: `app/src/components/authenticating-screen.tsx`

Shown during the OAuth flow after the user clicks "Connect with GitHub". Design shows: loader icon, "Authenticating..." title, "Waiting for GitHub authorization" subtitle, 3-step progress (Browser Opened → GitHub Auth → Receiving Token), info text, Cancel button, agent URL footer.

- [ ] **Step 1: Create authenticating-screen.tsx**

```tsx
import { Loader, Check, Globe, Github, Key } from "lucide-react";
import { Button } from "@/components/ui/button";

type AuthStep = "browser" | "auth" | "token";

export function AuthenticatingScreen({
    provider,
    agentUrl,
    step,
    onCancel,
}: {
    provider: string;
    agentUrl: string;
    step: AuthStep;
    onCancel: () => void;
}) {
    const providerLabel = provider.charAt(0).toUpperCase() + provider.slice(1);

    const steps: { id: AuthStep; label: string; icon: typeof Globe }[] = [
        { id: "browser", label: "Browser Opened", icon: Globe },
        { id: "auth", label: `${providerLabel} Auth`, icon: Github },
        { id: "token", label: "Receiving Token", icon: Key },
    ];

    const currentIdx = steps.findIndex((s) => s.id === step);

    return (
        <div className='flex flex-1 items-center justify-center'>
            <div className='flex w-[500px] flex-col items-center gap-6 border border-border bg-card p-10'>
                {/* Loader */}
                <Loader className='size-8 animate-spin text-accent-green' />

                {/* Title */}
                <div className='flex flex-col items-center gap-2'>
                    <h1 className='text-xl font-bold text-foreground'>Authenticating...</h1>
                    <p className='font-mono text-[13px] text-text-secondary'>
                        Waiting for {providerLabel} authorization
                    </p>
                </div>

                {/* Steps */}
                <div className='flex w-full items-center justify-center gap-6'>
                    {steps.map((s, i) => {
                        const isCompleted = i < currentIdx;
                        const isCurrent = i === currentIdx;
                        const Icon = isCompleted ? Check : s.icon;

                        return (
                            <div key={s.id} className='flex flex-col items-center gap-2'>
                                <Icon
                                    className={`size-5 ${
                                        isCompleted
                                            ? "text-accent-green"
                                            : isCurrent
                                              ? "animate-pulse text-accent-orange"
                                              : "text-muted-foreground"
                                    }`}
                                />
                                <span
                                    className={`font-mono text-[10px] font-semibold ${
                                        isCompleted
                                            ? "text-accent-green"
                                            : isCurrent
                                              ? "text-accent-orange"
                                              : "text-muted-foreground"
                                    }`}
                                >
                                    {s.label}
                                </span>
                            </div>
                        );
                    })}
                </div>

                {/* Info text */}
                <p className='text-center font-mono text-[11px] leading-relaxed text-muted-foreground'>
                    A browser window has been opened. Complete the authorization to continue.
                </p>

                {/* Cancel */}
                <Button variant='outline' size='sm' onClick={onCancel}>
                    Cancel
                </Button>

                {/* Agent URL */}
                <p className='font-mono text-[10px] text-muted-foreground'>
                    Connecting to {agentUrl}
                </p>
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Verify types pass**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 3: Commit**

```bash
git add app/src/components/authenticating-screen.tsx
git commit -m "feat: add authenticating transition screen with step progress"
```

---

### Task 6: Redesign Login/Connect Screen

**Files:**
- Modify: `app/src/components/login-screen.tsx`

The new design shows:
- Full dark background with accent line + titlebar (inherited from layout)
- Centered card with: logo section (container icon + "kontainer" + subtitle), agent URL input, "authenticate with" divider, "Connect with GitHub" green button, saved connections list, version footer
- The authenticating state shows the `AuthenticatingScreen` component instead of the form

This is the biggest UI change in this plan. The current login-screen.tsx has ~225 lines. The new one integrates the `AuthenticatingScreen` for the in-progress state.

- [ ] **Step 1: Read the current file**

Read `app/src/components/login-screen.tsx` to understand the full current implementation.

- [ ] **Step 2: Rewrite login-screen.tsx**

The new login screen should:
1. Keep all existing logic (handleConnect, handleUrlChange, handleProviderClick, setupRequired)
2. Add state for authenticating phase: `const [authenticatingProvider, setAuthenticatingProvider]` (already exists)
3. When `authenticatingProvider` is set, render `<AuthenticatingScreen>` instead of the form
4. Restyle the form to match the new design:
   - Dark card (`bg-card border border-border`) centered in viewport
   - Logo: container icon (56px, green, with accent border) + "kontainer" text (28px bold) + subtitle
   - "AGENT URL" label in mono uppercase
   - Input with link icon, elevated background
   - "authenticate with" divider
   - Green primary button "Connect with GitHub"
   - Saved connections section showing profiles from auth context

Key structure:
```tsx
// When authenticating, show transition screen
if (authenticatingProvider) {
    return (
        <AuthenticatingScreen
            provider={authenticatingProvider}
            agentUrl={agentUrl}
            step="auth"
            onCancel={() => setAuthenticatingProvider(null)}
        />
    );
}

// Otherwise show connect form
return (
    <div className='flex h-screen flex-col'>
        <div className='flex flex-1 items-center justify-center bg-background'>
            <div className='flex w-[480px] flex-col gap-8 border border-border bg-card p-10'>
                {/* Logo section */}
                {/* Connect form */}
                {/* Saved connections */}
                {/* Version footer */}
            </div>
        </div>
    </div>
);
```

The saved connections section should:
- Show profiles from `useAuth().profiles`
- Each connection: colored dot + label (bold) + URL (mono, muted) + chevron-right
- Clicking a connection calls `switchProfile(profile.id)` then navigates to "/"
- If no profiles: hide the section

Implement the full component following the design patterns from the Connect screen (`WDz3B` in design.pen). Use the design token reference from Plan 1 (mono fonts, muted colors, accent-green for primary actions).

- [ ] **Step 3: Update the login route layout**

The login route at `app/src/routes/login.tsx` currently renders `<LoginScreen>` directly. The new design shows the login screen has the accent line + titlebar at the top.

**IMPORTANT:** `DashboardHeader` calls `useCommandPalette()` which requires `CommandPaletteProvider` — that provider is only in `_authenticated.tsx`. Using `DashboardHeader` on the login route would crash. Instead, render a minimal header that just shows the logo.

```tsx
import { createFileRoute, redirect } from "@tanstack/react-router";
import { LoginScreen } from "@/components/login-screen";
import { AccentLine } from "@/components/accent-line";
import { Logo } from "@/components/logo";

export const Route = createFileRoute("/login")({
    validateSearch: (search: Record<string, unknown>) => ({
        redirect: (search.redirect as string) || undefined,
    }),
    beforeLoad: ({ context, search }) => {
        if (context.auth.user) {
            throw redirect({ to: search.redirect || "/" });
        }
    },
    component: LoginLayout,
});

function LoginLayout() {
    return (
        <div className='flex h-screen flex-col bg-background'>
            <AccentLine />
            <header data-tauri-drag-region className='flex h-[38px] shrink-0 items-center border-b border-border bg-titlebar-bg/90 px-4'>
                <div className='w-[52px] shrink-0' />
                <Logo />
            </header>
            <LoginScreen />
        </div>
    );
}
```

- [ ] **Step 4: Verify types pass**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 5: Run tests**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app test -- tests/unit/login-screen.spec.tsx
```

Fix any failures from the restructured component.

- [ ] **Step 6: Commit**

```bash
git add app/src/components/login-screen.tsx app/src/routes/login.tsx
git commit -m "feat: redesign connect screen with new dark theme and authenticating flow"
```

---

### Task 7: Redesign Cluster Switcher

**Files:**
- Modify: `app/src/components/cluster-switcher.tsx`

The new design (screen `syxFU`) shows the cluster switcher as a dropdown from the titlebar center area. It has:
- Header: "SWITCH CLUSTER" label + cluster count badge
- Cluster list: each cluster has a colored dot, name (bold), URL (mono muted), status label (Active/Healthy/Offline)
- "Add New Agent" button at bottom with cyan + icon

The current `ClusterSwitcher` is rendered in the sidebar bottom (now removed). It needs to be accessible from the titlebar's cluster name button. Currently the titlebar opens the command palette when clicked — we should change this to open the cluster switcher instead.

- [ ] **Step 1: Read the current cluster-switcher.tsx**

Read the current implementation to understand the profile management logic.

- [ ] **Step 2: Restyle ClusterSwitcher as a popover from titlebar**

Update the visual styling to match the design:
- Dark card background (`bg-card`)
- Border + shadow
- Header with mono "SWITCH CLUSTER" + count badge
- Each cluster row: dot (green for active, other colors for others) + label + URL + status
- "Add New Agent" button with cyan text

Keep all existing functionality: `handleSwitch`, `onRemove`, `onDisconnect`, `setDialogOpen`.

- [ ] **Step 3: Integrate into DashboardHeader**

Update `app/src/components/dashboard-header.tsx` to render `<ClusterSwitcher>` triggered by the cluster name button, instead of opening the command palette. The cluster name button in the center of the titlebar should toggle the switcher popover.

This means `DashboardHeader` will import and render `ClusterSwitcher` with the trigger being the cluster name area. The `useCommandPalette` import can stay for the search icon.

- [ ] **Step 4: Verify types pass**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 5: Commit**

```bash
git add app/src/components/cluster-switcher.tsx app/src/components/dashboard-header.tsx
git commit -m "feat: redesign cluster switcher dropdown from titlebar"
```

---

### Task 8: Update Tests

**Files:**
- Modify: `app/tests/unit/auth-context.spec.tsx`
- Modify: `app/tests/unit/login-screen.spec.tsx`

- [ ] **Step 1: Update auth-context tests for new handleToken flow**

The `handleToken` function now calls TWO commands instead of one:
1. `parse_auth_token` → returns `{ userId: "1", expiresAt: ... }`
2. `check_auth` → returns `{ user: { id, name, email, ... }, status: "ok" }`

Read `app/tests/unit/auth-context.spec.tsx`. Find and update:

**a)** Any `mockParsedToken` constant — change from:
```typescript
{ user: { id: "123", name: "Test", ... }, expiresAt: 9999999999 }
```
to:
```typescript
{ userId: "123", expiresAt: 9999999999 }
```

**b)** Add a `mockCheckAuthResponse` constant:
```typescript
const mockCheckAuthResponse = {
    user: { id: "123", name: "Test User", email: "test@test.com", avatarUrl: "", provider: "github" },
    status: "ok",
};
```

**c)** In every `mockImplementation` for `invoke`, add the `check_auth` handler alongside `parse_auth_token`:
```typescript
if (cmd === "parse_auth_token") return mockParsedToken;
if (cmd === "check_auth") return mockCheckAuthResponse;
```

**d)** Any error message assertion like `"Failed to parse auth token:"` should be updated to `"Failed to handle auth token:"` (the new catch block message).

- [ ] **Step 2: Update login-screen tests for redesigned UI**

Read `app/tests/unit/login-screen.spec.tsx`. Key selectors that will break:
- `screen.getByText("Connect to Cluster")` → update to new heading text (e.g., "kontainer")
- `screen.getByText("Enter your agent URL to get started.")` → update to new subtitle
- `screen.getByText("Authenticate")` → update to new divider text ("authenticate with")
- `screen.getByPlaceholderText("http://localhost:3000")` → verify placeholder still matches
- Any assertion about the authenticating state must account for `AuthenticatingScreen` being a separate component

Update each failing assertion to match the new DOM structure.

- [ ] **Step 3: Run all tests**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app test
```

Fix any failures.

- [ ] **Step 4: Commit**

```bash
git add app/tests/
git commit -m "test: update auth and login tests for new JWT flow and UI"
```

---

### Task 9: Final Verification

- [ ] **Step 1: Run type check**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 2: Run linter**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm lint
```

- [ ] **Step 3: Run formatter**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm fmt
```

- [ ] **Step 4: Run Go checks**

```bash
go vet -C /Users/clovismuneza/Apps/kontainer/server ./...
go test -C /Users/clovismuneza/Apps/kontainer/server ./...
```

- [ ] **Step 5: Run all frontend tests**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app test
```

- [ ] **Step 6: Run Rust check**

```bash
cargo check --manifest-path /Users/clovismuneza/Apps/kontainer/app/src-tauri/Cargo.toml
```

- [ ] **Step 7: Fix any issues and commit**

```bash
git add app/src/ app/tests/ server/
git commit -m "fix: resolve lint, format, and test issues from auth flow redesign"
```

- [ ] **Step 8: Format check**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm fmt
gofmt -l /Users/clovismuneza/Apps/kontainer/server/
cargo fmt --manifest-path /Users/clovismuneza/Apps/kontainer/app/src-tauri/Cargo.toml --check
```
