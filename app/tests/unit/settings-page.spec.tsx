import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { SettingsContext } from "@/contexts/settings-context-value";
import type { SettingsContextValue } from "@/contexts/settings-context-value";

vi.unmock("@/hooks/use-settings");

import { useAuth } from "@/hooks/use-auth";
vi.mock("@/hooks/use-auth", () => ({
    useAuth: vi.fn(),
}));

import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { GeneralTab } from "@/components/settings/general-tab";
import { AppearanceTab } from "@/components/settings/appearance-tab";
import { TerminalTab } from "@/components/settings/terminal-tab";
import { ConnectionTab } from "@/components/settings/connection-tab";

function SettingsTestComponent() {
    return (
        <div className='p-6'>
            <div className='mb-6'>
                <h1 className='text-xl font-semibold tracking-tight'>Settings</h1>
                <p className='mt-1 text-sm text-muted-foreground'>Application and cluster configuration</p>
            </div>

            <Tabs defaultValue='general'>
                <TabsList>
                    <TabsTrigger value='general'>General</TabsTrigger>
                    <TabsTrigger value='appearance'>Appearance</TabsTrigger>
                    <TabsTrigger value='terminal'>Terminal</TabsTrigger>
                    <TabsTrigger value='connection'>Connection</TabsTrigger>
                </TabsList>

                <TabsContent value='general'>
                    <GeneralTab />
                </TabsContent>
                <TabsContent value='appearance'>
                    <AppearanceTab />
                </TabsContent>
                <TabsContent value='terminal'>
                    <TerminalTab />
                </TabsContent>
                <TabsContent value='connection'>
                    <ConnectionTab />
                </TabsContent>
            </Tabs>
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

    it("renders all four tab triggers", async () => {
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("General")).toBeInTheDocument();
            expect(screen.getByRole("tab", { name: "Appearance" })).toBeInTheDocument();
            expect(screen.getByText("Terminal")).toBeInTheDocument();
            expect(screen.getByRole("tab", { name: "Connection" })).toBeInTheDocument();
        });
    });

    it("shows General tab content by default", async () => {
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("App Settings")).toBeInTheDocument();
            expect(screen.getByText("Configure how the desktop app behaves")).toBeInTheDocument();
        });
    });

    it("renders refresh interval label in General tab", async () => {
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

async function waitForInitialRender() {
    await waitFor(() => {
        expect(screen.getByText("Settings")).toBeInTheDocument();
    });
}

async function clickTab(name: string) {
    await waitForInitialRender();
    await act(async () => {
        screen.getByRole("tab", { name }).click();
    });
}

describe("SettingsPage - Appearance tab", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.mocked(useAuth).mockReturnValue(defaultAuthReturn);
    });

    it("renders Appearance card when tab is clicked", async () => {
        renderWithProviders();
        await clickTab("Appearance");

        await waitFor(() => {
            expect(screen.getByText("Customize how the app looks")).toBeInTheDocument();
        });
    });

    it("shows font selector label", async () => {
        renderWithProviders();
        await clickTab("Appearance");

        await waitFor(() => {
            expect(screen.getByText("Font")).toBeInTheDocument();
            expect(screen.getByText("The typeface used across the app UI")).toBeInTheDocument();
        });
    });

    it("renders font select trigger", async () => {
        renderWithProviders();
        await clickTab("Appearance");

        await waitFor(() => {
            expect(screen.getByLabelText("Font")).toBeInTheDocument();
        });
    });
});

describe("SettingsPage - Terminal tab", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.mocked(useAuth).mockReturnValue(defaultAuthReturn);
    });

    it("renders Terminal Defaults card when tab is clicked", async () => {
        renderWithProviders();
        await clickTab("Terminal");

        await waitFor(() => {
            expect(screen.getByText("Terminal Defaults")).toBeInTheDocument();
            expect(screen.getByText("Default settings for terminal sessions")).toBeInTheDocument();
        });
    });

    it("shows default shell as Auto-detect", async () => {
        renderWithProviders();
        await clickTab("Terminal");

        await waitFor(() => {
            expect(screen.getByText("Auto-detect")).toBeInTheDocument();
        });
    });

    it("shows default font size as 13", async () => {
        renderWithProviders();
        await clickTab("Terminal");

        await waitFor(() => {
            expect(screen.getByText("13")).toBeInTheDocument();
        });
    });

    it("shows default scrollback as 1,000", async () => {
        renderWithProviders();
        await clickTab("Terminal");

        await waitFor(() => {
            expect(screen.getByText("1,000")).toBeInTheDocument();
        });
    });
});

describe("SettingsPage - Connection tab", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.mocked(useAuth).mockReturnValue(defaultAuthReturn);
    });

    it("renders Connection card when tab is clicked", async () => {
        renderWithProviders();
        await clickTab("Connection");

        await waitFor(() => {
            expect(screen.getByText("Manage the active agent connection")).toBeInTheDocument();
        });
    });

    it("shows active profile label and agent URL", async () => {
        renderWithProviders();
        await clickTab("Connection");

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
        await clickTab("Connection");

        await waitFor(() => {
            expect(screen.getByText("No active connection. Connect to a cluster first.")).toBeInTheDocument();
        });
    });

    it("disables save button when no changes", async () => {
        renderWithProviders();
        await clickTab("Connection");

        await waitFor(() => {
            const saveButton = screen.getByRole("button", { name: "Save" });
            expect(saveButton).toBeDisabled();
        });
    });
});
