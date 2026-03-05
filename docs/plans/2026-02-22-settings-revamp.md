# Settings Page Revamp Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the tab-and-card settings page with a scrollable full-page sections layout that feels like a native desktop app.

**Architecture:** Flatten all four tab components into section components rendered sequentially in a single scrollable page. Remove Tabs and Card wrappers entirely. Each section gets a monospaced uppercase heading with a border separator.

**Tech Stack:** React, Tailwind CSS 4, shadcn/ui (Select, Input, Button, Label, Separator), TanStack Router

---

### Task 1: Rewrite GeneralSection (remove Card wrapper)

**Files:**
- Modify: `app/src/components/settings/general-tab.tsx`

**Step 1: Rewrite component to flat section layout**

Replace the entire file with:

```tsx
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useSettings } from "@/hooks/use-settings";

const REFRESH_OPTIONS = [
    { value: "3000", label: "3 seconds" },
    { value: "5000", label: "5 seconds" },
    { value: "10000", label: "10 seconds" },
    { value: "30000", label: "30 seconds" },
    { value: "false", label: "Off" },
] as const;

export function GeneralSection() {
    const { refreshInterval, setRefreshInterval } = useSettings();

    const currentValue = refreshInterval === false ? "false" : String(refreshInterval);

    function handleRefreshChange(value: string | null) {
        if (!value) return;
        if (value === "false") {
            setRefreshInterval(false);
        } else {
            setRefreshInterval(Number(value));
        }
    }

    return (
        <section>
            <h2 className="mb-3 border-b border-border pb-2 font-mono text-xs uppercase tracking-widest text-muted-foreground">
                General
            </h2>
            <div className="flex items-center justify-between py-3">
                <div className="space-y-0.5">
                    <Label htmlFor="refresh-interval">Refresh interval</Label>
                    <p className="text-xs text-muted-foreground">How often data is polled from the agent</p>
                </div>
                <Select value={currentValue} onValueChange={handleRefreshChange} items={REFRESH_OPTIONS}>
                    <SelectTrigger id="refresh-interval" className="w-36">
                        <SelectValue placeholder="Select interval" />
                    </SelectTrigger>
                    <SelectContent>
                        {REFRESH_OPTIONS.map((opt) => (
                            <SelectItem key={opt.value} value={opt.value}>
                                {opt.label}
                            </SelectItem>
                        ))}
                    </SelectContent>
                </Select>
            </div>
        </section>
    );
}
```

**Step 2: Verify it compiles**

Run: `pnpm --filter app types`
Expected: No errors related to general-tab.tsx

### Task 2: Rewrite AppearanceSection (remove Card wrapper)

**Files:**
- Modify: `app/src/components/settings/appearance-tab.tsx`

**Step 1: Rewrite component to flat section layout**

Replace the entire file with:

```tsx
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useAppearance } from "@/hooks/use-appearance";

export function AppearanceSection() {
    const { font, setFont, fonts } = useAppearance();

    return (
        <section>
            <h2 className="mb-3 border-b border-border pb-2 font-mono text-xs uppercase tracking-widest text-muted-foreground">
                Appearance
            </h2>
            <div className="flex items-center justify-between py-3">
                <div className="space-y-0.5">
                    <Label htmlFor="font-select">Font</Label>
                    <p className="text-xs text-muted-foreground">The typeface used across the app UI</p>
                </div>
                <Select value={font} onValueChange={(v) => v && setFont(v)}>
                    <SelectTrigger id="font-select" className="w-48">
                        <SelectValue placeholder="Select font" />
                    </SelectTrigger>
                    <SelectContent>
                        {fonts.map((f) => (
                            <SelectItem key={f.id} value={f.id}>
                                {f.label}
                            </SelectItem>
                        ))}
                    </SelectContent>
                </Select>
            </div>
        </section>
    );
}
```

**Step 2: Verify it compiles**

Run: `pnpm --filter app types`
Expected: No errors related to appearance-tab.tsx

### Task 3: Rewrite TerminalSection (remove Card wrapper)

**Files:**
- Modify: `app/src/components/settings/terminal-tab.tsx`

**Step 1: Rewrite component to flat section layout**

Replace the entire file with:

```tsx
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useSettings } from "@/hooks/use-settings";

const SHELL_OPTIONS = [
    { value: "auto", label: "Auto-detect" },
    { value: "/bin/bash", label: "/bin/bash" },
    { value: "/bin/sh", label: "/bin/sh" },
    { value: "/bin/zsh", label: "/bin/zsh" },
] as const;

const FONT_SIZE_OPTIONS = [
    { value: "11", label: "11" },
    { value: "12", label: "12" },
    { value: "13", label: "13" },
    { value: "14", label: "14" },
    { value: "15", label: "15" },
    { value: "16", label: "16" },
    { value: "18", label: "18" },
    { value: "20", label: "20" },
] as const;

const SCROLLBACK_OPTIONS = [
    { value: "500", label: "500" },
    { value: "1000", label: "1,000" },
    { value: "2500", label: "2,500" },
    { value: "5000", label: "5,000" },
    { value: "10000", label: "10,000" },
] as const;

export function TerminalSection() {
    const { terminalDefaults, setTerminalDefaults } = useSettings();

    return (
        <section>
            <h2 className="mb-3 border-b border-border pb-2 font-mono text-xs uppercase tracking-widest text-muted-foreground">
                Terminal
            </h2>
            <div className="divide-y divide-border/50">
                <div className="flex items-center justify-between py-3">
                    <div className="space-y-0.5">
                        <Label htmlFor="terminal-shell">Default shell</Label>
                        <p className="text-xs text-muted-foreground">Fallback shell when none is specified</p>
                    </div>
                    <Select
                        items={SHELL_OPTIONS}
                        value={terminalDefaults.shell || "auto"}
                        onValueChange={(v) => setTerminalDefaults({ shell: v === "auto" || !v ? "" : v })}
                    >
                        <SelectTrigger id="terminal-shell" className="w-36">
                            <SelectValue placeholder="Select shell" />
                        </SelectTrigger>
                        <SelectContent>
                            {SHELL_OPTIONS.map((opt) => (
                                <SelectItem key={opt.value} value={opt.value}>
                                    {opt.label}
                                </SelectItem>
                            ))}
                        </SelectContent>
                    </Select>
                </div>

                <div className="flex items-center justify-between py-3">
                    <div className="space-y-0.5">
                        <Label htmlFor="terminal-font-size">Font size</Label>
                        <p className="text-xs text-muted-foreground">Terminal text size in pixels</p>
                    </div>
                    <Select
                        items={FONT_SIZE_OPTIONS}
                        value={String(terminalDefaults.fontSize)}
                        onValueChange={(v) => setTerminalDefaults({ fontSize: Number(v) })}
                    >
                        <SelectTrigger id="terminal-font-size" className="w-36">
                            <SelectValue placeholder="Select size" />
                        </SelectTrigger>
                        <SelectContent>
                            {FONT_SIZE_OPTIONS.map((opt) => (
                                <SelectItem key={opt.value} value={opt.value}>
                                    {opt.label}
                                </SelectItem>
                            ))}
                        </SelectContent>
                    </Select>
                </div>

                <div className="flex items-center justify-between py-3">
                    <div className="space-y-0.5">
                        <Label htmlFor="terminal-font-family">Font family</Label>
                        <p className="text-xs text-muted-foreground">CSS font-family for the terminal</p>
                    </div>
                    <Input
                        id="terminal-font-family"
                        className="w-64"
                        value={terminalDefaults.fontFamily}
                        placeholder="'JetBrains Mono', monospace"
                        onChange={(e) => setTerminalDefaults({ fontFamily: e.target.value })}
                    />
                </div>

                <div className="flex items-center justify-between py-3">
                    <div className="space-y-0.5">
                        <Label htmlFor="terminal-scrollback">Scrollback buffer</Label>
                        <p className="text-xs text-muted-foreground">Number of lines kept in scroll history</p>
                    </div>
                    <Select
                        items={SCROLLBACK_OPTIONS}
                        value={String(terminalDefaults.scrollback)}
                        onValueChange={(v) => setTerminalDefaults({ scrollback: Number(v) })}
                    >
                        <SelectTrigger id="terminal-scrollback" className="w-36">
                            <SelectValue placeholder="Select size" />
                        </SelectTrigger>
                        <SelectContent>
                            {SCROLLBACK_OPTIONS.map((opt) => (
                                <SelectItem key={opt.value} value={opt.value}>
                                    {opt.label}
                                </SelectItem>
                            ))}
                        </SelectContent>
                    </Select>
                </div>
            </div>
        </section>
    );
}
```

**Step 2: Verify it compiles**

Run: `pnpm --filter app types`
Expected: No errors related to terminal-tab.tsx

### Task 4: Rewrite ConnectionSection (remove Card wrapper, keep Save)

**Files:**
- Modify: `app/src/components/settings/connection-tab.tsx`

**Step 1: Rewrite component to flat section layout**

Replace the entire file with:

```tsx
import { useEffect, useState } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { useAuth } from "@/hooks/use-auth";

export function ConnectionSection() {
    const { profiles, activeProfileId, updateProfile } = useAuth();
    const activeProfile = profiles.find((p) => p.id === activeProfileId);

    const [label, setLabel] = useState(activeProfile?.label ?? "");
    const [agentUrl, setAgentUrl] = useState(activeProfile?.agentUrl ?? "");
    const [saving, setSaving] = useState(false);

    useEffect(() => {
        setLabel(activeProfile?.label ?? "");
        setAgentUrl(activeProfile?.agentUrl ?? "");
    }, [activeProfile?.id, activeProfile?.label, activeProfile?.agentUrl]);

    const hasChanges = activeProfile
        ? label !== activeProfile.label || agentUrl !== activeProfile.agentUrl
        : false;

    async function handleSave() {
        if (!activeProfile || !hasChanges) return;
        setSaving(true);
        try {
            await updateProfile(activeProfile.id, { label, agentUrl });
        } catch (err) {
            console.error("Failed to update profile:", err);
        } finally {
            setSaving(false);
        }
    }

    return (
        <section>
            <h2 className="mb-3 border-b border-border pb-2 font-mono text-xs uppercase tracking-widest text-muted-foreground">
                Connection
            </h2>
            {!activeProfile ? (
                <p className="py-3 text-sm text-muted-foreground">
                    No active connection. Connect to a cluster first.
                </p>
            ) : (
                <div className="divide-y divide-border/50">
                    <div className="flex items-center justify-between py-3">
                        <div className="space-y-0.5">
                            <Label htmlFor="profile-label">Display name</Label>
                            <p className="text-xs text-muted-foreground">A friendly name for this cluster</p>
                        </div>
                        <Input
                            id="profile-label"
                            className="w-64"
                            value={label}
                            placeholder="e.g. Production"
                            onChange={(e) => setLabel(e.target.value)}
                        />
                    </div>

                    <div className="flex items-center justify-between py-3">
                        <div className="space-y-0.5">
                            <Label htmlFor="agent-url">Agent URL</Label>
                            <p className="text-xs text-muted-foreground">Endpoint for this cluster&apos;s agent</p>
                        </div>
                        <Input
                            id="agent-url"
                            className="w-64"
                            value={agentUrl}
                            placeholder="https://agent.example.com"
                            onChange={(e) => setAgentUrl(e.target.value)}
                        />
                    </div>

                    <div className="flex justify-end pt-3">
                        <Button size="sm" disabled={!hasChanges || saving} onClick={handleSave}>
                            {saving ? "Saving..." : "Save"}
                        </Button>
                    </div>
                </div>
            )}
        </section>
    );
}
```

**Step 2: Verify it compiles**

Run: `pnpm --filter app types`
Expected: No errors related to connection-tab.tsx

### Task 5: Rewrite settings page route (remove Tabs, compose sections)

**Files:**
- Modify: `app/src/routes/_authenticated/settings/index.tsx`

**Step 1: Rewrite route to compose all sections**

Replace the entire file with:

```tsx
import { createFileRoute } from "@tanstack/react-router";
import { GeneralSection } from "@/components/settings/general-tab";
import { AppearanceSection } from "@/components/settings/appearance-tab";
import { TerminalSection } from "@/components/settings/terminal-tab";
import { ConnectionSection } from "@/components/settings/connection-tab";

function SettingsPage() {
    return (
        <div className="mx-auto max-w-2xl px-6 py-6">
            <div className="mb-8">
                <h1 className="text-xl font-semibold tracking-tight">Settings</h1>
                <p className="mt-1 text-sm text-muted-foreground">
                    Application and cluster configuration
                </p>
            </div>

            <div className="space-y-8">
                <GeneralSection />
                <AppearanceSection />
                <TerminalSection />
                <ConnectionSection />
            </div>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/settings/")({
    component: SettingsPage,
});
```

**Step 2: Verify it compiles**

Run: `pnpm --filter app types`
Expected: No errors

### Task 6: Update tests to match new structure

**Files:**
- Modify: `app/tests/unit/settings-page.spec.tsx`

The test file currently:
- Imports `Tabs`, `TabsList`, `TabsTrigger`, `TabsContent` and renders a `SettingsTestComponent` that mirrors the old page
- Uses `clickTab()` helpers to switch between tabs
- Asserts on Card titles like "App Settings", "Terminal Defaults"

Since the new page has no tabs (all sections visible at once), the tests simplify:

**Step 1: Rewrite the test file**

Replace the entire file with:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { SettingsContext } from "@/contexts/settings-context-value";
import type { SettingsContextValue } from "@/contexts/settings-context-value";

vi.unmock("@/hooks/use-settings");

import { useAuth } from "@/hooks/use-auth";
vi.mock("@/hooks/use-auth", () => ({
    useAuth: vi.fn(),
}));

import { GeneralSection } from "@/components/settings/general-tab";
import { AppearanceSection } from "@/components/settings/appearance-tab";
import { TerminalSection } from "@/components/settings/terminal-tab";
import { ConnectionSection } from "@/components/settings/connection-tab";

function SettingsTestComponent() {
    return (
        <div className="mx-auto max-w-2xl px-6 py-6">
            <div className="mb-8">
                <h1 className="text-xl font-semibold tracking-tight">Settings</h1>
                <p className="mt-1 text-sm text-muted-foreground">
                    Application and cluster configuration
                </p>
            </div>
            <div className="space-y-8">
                <GeneralSection />
                <AppearanceSection />
                <TerminalSection />
                <ConnectionSection />
            </div>
        </div>
    );
}

function createTestQueryClient() {
    return new QueryClient({
        defaultOptions: {
            queries: { retry: false, gcTime: 0 },
        },
    });
}

const mockSettings: SettingsContextValue = {
    refreshInterval: 5000,
    setRefreshInterval: vi.fn(),
    terminalDefaults: {
        shell: "",
        fontSize: 13,
        fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', Menlo, Monaco, 'Courier New', monospace",
        scrollback: 1000,
    },
    setTerminalDefaults: vi.fn(),
    isLoading: false,
};

const defaultAuthReturn = {
    user: { id: "u1", name: "Test User", email: "test@example.com", avatarUrl: "", provider: "github" },
    token: "test-token",
    expiresAt: Math.floor(Date.now() / 1000) + 3600,
    agentUrl: "https://agent.example.com",
    isLoading: false,
    profiles: [
        {
            id: "p1",
            label: "Production",
            agentUrl: "https://agent.example.com",
            userId: "u1",
            userName: "Test User",
            userEmail: "test@example.com",
            userAvatarUrl: "",
            userProvider: "github",
            token: "test-token",
            expiresAt: Math.floor(Date.now() / 1000) + 3600,
        },
    ],
    activeProfileId: "p1",
    login: vi.fn(),
    logout: vi.fn(),
    switchProfile: vi.fn(),
    removeProfile: vi.fn(),
    updateProfile: vi.fn(),
};

function createTestRouter() {
    const rootRoute = createRootRoute({});
    const indexRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/",
        component: SettingsTestComponent,
    });
    const routeTree = rootRoute.addChildren([indexRoute]);
    return createRouter({
        routeTree,
        history: createMemoryHistory({ initialEntries: ["/"] }),
    });
}

function renderWithProviders(settingsOverrides: Partial<SettingsContextValue> = {}) {
    const queryClient = createTestQueryClient();
    const settings = { ...mockSettings, ...settingsOverrides };
    const router = createTestRouter();

    return render(
        <SettingsContext value={settings}>
            <QueryClientProvider client={queryClient}>
                <RouterProvider router={router} />
            </QueryClientProvider>
        </SettingsContext>,
    );
}

describe("SettingsPage", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.mocked(useAuth).mockReturnValue(defaultAuthReturn);
    });

    it("renders the page heading", async () => {
        renderWithProviders();
        await waitFor(() => {
            expect(screen.getByText("Settings")).toBeInTheDocument();
        });
    });

    it("renders all section headings", async () => {
        renderWithProviders();
        await waitFor(() => {
            expect(screen.getByText("General")).toBeInTheDocument();
            expect(screen.getByText("Appearance")).toBeInTheDocument();
            expect(screen.getByText("Terminal")).toBeInTheDocument();
            expect(screen.getByText("Connection")).toBeInTheDocument();
        });
    });
});

describe("SettingsPage - General section", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.mocked(useAuth).mockReturnValue(defaultAuthReturn);
    });

    it("renders refresh interval label", async () => {
        renderWithProviders();
        await waitFor(() => {
            expect(screen.getByText("Refresh interval")).toBeInTheDocument();
            expect(screen.getByText("How often data is polled from the agent")).toBeInTheDocument();
        });
    });

    it("shows current refresh interval value", async () => {
        renderWithProviders({ refreshInterval: 5000 });
        await waitFor(() => {
            expect(screen.getByText("5 seconds")).toBeInTheDocument();
        });
    });

    it("shows Off when polling is disabled", async () => {
        renderWithProviders({ refreshInterval: false });
        await waitFor(() => {
            expect(screen.getByText("Off")).toBeInTheDocument();
        });
    });

    it("shows 3 seconds option for 3000ms interval", async () => {
        renderWithProviders({ refreshInterval: 3000 });
        await waitFor(() => {
            expect(screen.getByText("3 seconds")).toBeInTheDocument();
        });
    });

    it("shows 30 seconds option for 30000ms interval", async () => {
        renderWithProviders({ refreshInterval: 30000 });
        await waitFor(() => {
            expect(screen.getByText("30 seconds")).toBeInTheDocument();
        });
    });
});

describe("SettingsPage - Appearance section", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.mocked(useAuth).mockReturnValue(defaultAuthReturn);
    });

    it("shows font selector label", async () => {
        renderWithProviders();
        await waitFor(() => {
            expect(screen.getByText("Font")).toBeInTheDocument();
            expect(screen.getByText("The typeface used across the app UI")).toBeInTheDocument();
        });
    });

    it("renders font select trigger", async () => {
        renderWithProviders();
        await waitFor(() => {
            expect(screen.getByLabelText("Font")).toBeInTheDocument();
        });
    });
});

describe("SettingsPage - Terminal section", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.mocked(useAuth).mockReturnValue(defaultAuthReturn);
    });

    it("shows default shell as Auto-detect", async () => {
        renderWithProviders();
        await waitFor(() => {
            expect(screen.getByText("Auto-detect")).toBeInTheDocument();
        });
    });

    it("shows default font size as 13", async () => {
        renderWithProviders();
        await waitFor(() => {
            expect(screen.getByText("13")).toBeInTheDocument();
        });
    });

    it("shows default scrollback as 1,000", async () => {
        renderWithProviders();
        await waitFor(() => {
            expect(screen.getByText("1,000")).toBeInTheDocument();
        });
    });
});

describe("SettingsPage - Connection section", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.mocked(useAuth).mockReturnValue(defaultAuthReturn);
    });

    it("shows active profile label and agent URL", async () => {
        renderWithProviders();
        await waitFor(() => {
            const labelInput = screen.getByLabelText("Display name") as HTMLInputElement;
            const urlInput = screen.getByLabelText("Agent URL") as HTMLInputElement;
            expect(labelInput.value).toBe("Production");
            expect(urlInput.value).toBe("https://agent.example.com");
        });
    });

    it("shows no-connection message when no active profile", async () => {
        vi.mocked(useAuth).mockReturnValue({
            ...defaultAuthReturn,
            profiles: [],
            activeProfileId: null,
        });

        renderWithProviders();
        await waitFor(() => {
            expect(screen.getByText("No active connection. Connect to a cluster first.")).toBeInTheDocument();
        });
    });

    it("disables save button when no changes", async () => {
        renderWithProviders();
        await waitFor(() => {
            const saveButton = screen.getByRole("button", { name: "Save" });
            expect(saveButton).toBeDisabled();
        });
    });
});
```

**Step 2: Run tests**

Run: `pnpm --filter app test tests/unit/settings-page.spec.tsx`
Expected: All tests pass

### Task 7: Run lint, types, and format

**Step 1: Type check**

Run: `pnpm types`
Expected: No errors

**Step 2: Lint**

Run: `pnpm lint`
Expected: No errors

**Step 3: Format**

Run: `pnpm fmt`
Expected: Files formatted

### Task 8: Commit

**Step 1: Commit all changes**

```bash
git add app/src/routes/_authenticated/settings/index.tsx \
       app/src/components/settings/general-tab.tsx \
       app/src/components/settings/appearance-tab.tsx \
       app/src/components/settings/terminal-tab.tsx \
       app/src/components/settings/connection-tab.tsx \
       app/tests/unit/settings-page.spec.tsx
git commit -m "refactor: replace tab-and-card settings with full-page sections layout

Remove horizontal Tabs and Card wrappers from the settings page.
All sections now render sequentially in a scrollable page with
monospaced uppercase headings and thin separators. Feels like a
native desktop app instead of a web SaaS dashboard."
```
