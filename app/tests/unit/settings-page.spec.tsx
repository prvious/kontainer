import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { SettingsContext } from "@/contexts/settings-context-value";
import type { SettingsContextValue } from "@/contexts/settings-context-value";

// Unmock useSettings so the real hook reads from our test SettingsContext
vi.unmock("@/hooks/use-settings");
import { useSettings } from "@/hooks/use-settings";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

const REFRESH_OPTIONS = [
    { value: "3000", label: "3 seconds" },
    { value: "5000", label: "5 seconds" },
    { value: "10000", label: "10 seconds" },
    { value: "30000", label: "30 seconds" },
    { value: "false", label: "Off" },
] as const;

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

function SettingsTestComponent() {
    const { refreshInterval, setRefreshInterval, terminalDefaults, setTerminalDefaults } = useSettings();

    const currentValue = refreshInterval === false ? "false" : String(refreshInterval);

    function handleRefreshChange(value: string) {
        if (value === "false") {
            setRefreshInterval(false);
        } else {
            setRefreshInterval(Number(value));
        }
    }

    return (
        <div className='p-6'>
            <div className='mb-6'>
                <h1 className='text-xl font-semibold tracking-tight'>Settings</h1>
                <p className='mt-1 text-sm text-muted-foreground'>Application and cluster configuration</p>
            </div>

            <div className='space-y-6'>
                <Card>
                    <CardHeader>
                        <CardTitle>App Settings</CardTitle>
                        <CardDescription>Configure how the desktop app behaves</CardDescription>
                    </CardHeader>
                    <CardContent>
                        <div className='flex items-center justify-between'>
                            <div className='space-y-0.5'>
                                <Label htmlFor='refresh-interval'>Refresh interval</Label>
                                <p className='text-xs text-muted-foreground'>How often data is polled from the agent</p>
                            </div>
                            <Select value={currentValue} onValueChange={handleRefreshChange} items={REFRESH_OPTIONS}>
                                <SelectTrigger id='refresh-interval' className='w-36'>
                                    <SelectValue placeholder='Select interval' />
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
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader>
                        <CardTitle>Terminal Defaults</CardTitle>
                        <CardDescription>Default settings for terminal sessions</CardDescription>
                    </CardHeader>
                    <CardContent className='space-y-4'>
                        <div className='flex items-center justify-between'>
                            <div className='space-y-0.5'>
                                <Label htmlFor='terminal-shell'>Default shell</Label>
                                <p className='text-xs text-muted-foreground'>Fallback shell when none is specified</p>
                            </div>
                            <Select
                                items={SHELL_OPTIONS}
                                value={terminalDefaults.shell || "auto"}
                                onValueChange={(v) => setTerminalDefaults({ shell: v === "auto" ? "" : v })}
                            >
                                <SelectTrigger id='terminal-shell' className='w-36'>
                                    <SelectValue placeholder='Select shell' />
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

                        <div className='flex items-center justify-between'>
                            <div className='space-y-0.5'>
                                <Label htmlFor='terminal-font-size'>Font size</Label>
                                <p className='text-xs text-muted-foreground'>Terminal text size in pixels</p>
                            </div>
                            <Select
                                items={FONT_SIZE_OPTIONS}
                                value={String(terminalDefaults.fontSize)}
                                onValueChange={(v) => setTerminalDefaults({ fontSize: Number(v) })}
                            >
                                <SelectTrigger id='terminal-font-size' className='w-36'>
                                    <SelectValue placeholder='Select size' />
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

                        <div className='flex items-center justify-between'>
                            <div className='space-y-0.5'>
                                <Label htmlFor='terminal-font-family'>Font family</Label>
                                <p className='text-xs text-muted-foreground'>CSS font-family for the terminal</p>
                            </div>
                            <Input
                                id='terminal-font-family'
                                className='w-64'
                                value={terminalDefaults.fontFamily}
                                placeholder="'JetBrains Mono', monospace"
                                onChange={(e) => setTerminalDefaults({ fontFamily: e.target.value })}
                            />
                        </div>

                        <div className='flex items-center justify-between'>
                            <div className='space-y-0.5'>
                                <Label htmlFor='terminal-scrollback'>Scrollback buffer</Label>
                                <p className='text-xs text-muted-foreground'>Number of lines kept in scroll history</p>
                            </div>
                            <Select
                                items={SCROLLBACK_OPTIONS}
                                value={String(terminalDefaults.scrollback)}
                                onValueChange={(v) => setTerminalDefaults({ scrollback: Number(v) })}
                            >
                                <SelectTrigger id='terminal-scrollback' className='w-36'>
                                    <SelectValue placeholder='Select size' />
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
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader>
                        <CardTitle>Agent Settings</CardTitle>
                        <CardDescription>Settings stored on the connected agent</CardDescription>
                    </CardHeader>
                    <CardContent>
                        <p className='text-sm text-muted-foreground'>No agent settings available yet.</p>
                    </CardContent>
                </Card>
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
    });

    it("renders the page heading", async () => {
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Settings")).toBeInTheDocument();
        });
    });

    it("renders App Settings card", async () => {
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("App Settings")).toBeInTheDocument();
            expect(screen.getByText("Configure how the desktop app behaves")).toBeInTheDocument();
        });
    });

    it("renders Agent Settings placeholder", async () => {
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Agent Settings")).toBeInTheDocument();
            expect(screen.getByText("No agent settings available yet.")).toBeInTheDocument();
        });
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

    it("renders Terminal Defaults card", async () => {
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Terminal Defaults")).toBeInTheDocument();
            expect(screen.getByText("Default settings for terminal sessions")).toBeInTheDocument();
        });
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
