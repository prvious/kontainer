import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { useSettings } from "@/hooks/use-settings";

type SettingsValue = ReturnType<typeof useSettings>;

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
        <div className='mx-auto max-w-2xl px-6 py-6'>
            <div className='mb-8'>
                <h1 className='text-xl font-semibold tracking-tight'>Settings</h1>
                <p className='mt-1 text-sm text-muted-foreground'>Application and cluster configuration</p>
            </div>
            <div className='space-y-8'>
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

const mockSettings: SettingsValue = {
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
    session: {
        profileId: "p1",
        origin: "https://controller.example",
        clusterId: "cluster-1",
        principalId: "u1",
        githubLogin: "test",
        label: "Production",
        accessExpiresAtUnixMs: 2_000_000_000_000,
    },
    issue: null,
    restoreError: null,
    isLoading: false,
    login: vi.fn(),
    logout: vi.fn(),
    retryCommit: vi.fn(),
    retryRestore: vi.fn(),
    cancelLogin: vi.fn(),
    clearIssue: vi.fn(),
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

function renderWithProviders(settingsOverrides: Partial<SettingsValue> = {}) {
    const queryClient = createTestQueryClient();
    vi.mocked(useSettings).mockReturnValue({ ...mockSettings, ...settingsOverrides });
    const router = createTestRouter();

    return render(
        <QueryClientProvider client={queryClient}>
            <RouterProvider router={router} />
        </QueryClientProvider>,
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

    it("renders refresh interval label in General section", async () => {
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

    it("offers the theme instead of a font picker", async () => {
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByLabelText("Theme")).toBeInTheDocument();
            expect(screen.getByText("System")).toBeInTheDocument();
        });
        expect(screen.queryByText("Font")).not.toBeInTheDocument();
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

    it("shows active profile label and controller URL", async () => {
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Production")).toBeInTheDocument();
            expect(screen.getByText("https://controller.example")).toBeInTheDocument();
        });
    });

    it("shows no-connection message when no active profile", async () => {
        vi.mocked(useAuth).mockReturnValue({
            ...defaultAuthReturn,
            session: null,
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No active connection. Connect to a controller first.")).toBeInTheDocument();
        });
    });

    it("does not expose plaintext profile editing", async () => {
        renderWithProviders();

        await waitFor(() => {
            expect(screen.queryByRole("button", { name: "Save" })).not.toBeInTheDocument();
            expect(screen.queryByRole("textbox", { name: "Display name" })).not.toBeInTheDocument();
        });
    });
});
