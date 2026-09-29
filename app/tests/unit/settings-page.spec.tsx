import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRouter, RouterProvider } from "@tanstack/react-router";
import { routeTree } from "@/routeTree.gen";
import { AuthContext, type AuthContextValue } from "@/contexts/auth-context-value";
import { AppearanceProvider } from "@/contexts/appearance-context";
import { REFRESH_INTERVALS, changedRows, draftFrom, fieldErrors, withCurrent } from "@/components/settings/settings-form";
import { controllerStatusFromIpc } from "@/types/nodes";
import { DEFAULT_APP_SETTINGS, type AppSettings } from "@/types/settings";
import nodes from "../fixtures/nodes-ipc.json";

vi.unmock("@/hooks/use-appearance");
const toasts = vi.hoisted(() => ({ success: vi.fn(), error: vi.fn() }));
vi.mock("sonner", () => ({
    toast: Object.assign(vi.fn(), { success: toasts.success, error: toasts.error, warning: vi.fn(), loading: vi.fn(), dismiss: vi.fn() }),
    Toaster: () => null,
}));

const session = {
    profileId: "018f2e34-0000-7000-8000-0000000000bb",
    origin: "https://controller.example/",
    clusterId: "cluster",
    principalId: "principal",
    email: "owner@example.com",
    displayName: "Owner",
    label: "Production",
    accessExpiresAtUnixMs: 2_000_000_000_000,
};
const logout = vi.fn(async () => {});
const baseAuth = {
    isLoading: false,
    expired: false,
    issue: null,
    reportInventoryCustodyError: vi.fn(),
    retryCommit: vi.fn(),
    retryRestore: vi.fn(),
};

let stored: AppSettings;
let retention: number;

function mockCommands() {
    stored = { ...DEFAULT_APP_SETTINGS, refreshInterval: 5_000, scheme: "dark", terminalFontSize: 14, terminalScrollback: 10_000 };
    retention = 7;
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
        switch (command) {
            case "load_app_settings":
                return stored;
            case "save_app_settings":
                stored = (args as { settings: AppSettings }).settings;
                return null;
            case "get_settings":
                return { metricsRetentionDays: retention };
            case "update_settings":
                retention = (args as { metricsRetentionDays: number }).metricsRetentionDays;
                return { metricsRetentionDays: retention };
            case "get_status":
                return nodes.status;
            case "controller_health":
                return { status: "ready" };
            case "session_connections":
                return [];
            default:
                return null;
        }
    });
}

type TestRouter = { state: { location: { pathname: string; search: unknown } } };

/** Like `app.tsx`: auth lives in state, and signing out clears the session the router sees. */
function Harness({ onRouter }: { onRouter: (router: TestRouter) => void }) {
    const [signedIn, setSignedIn] = useState(true);
    const [nextOrigin, setNextOrigin] = useState<string | null>(null);
    const auth = {
        ...baseAuth,
        session: signedIn ? session : null,
        nextOrigin,
        logout: async (_summary: unknown, next?: string) => {
            await logout();
            setNextOrigin(next ?? null);
            setSignedIn(false);
        },
    } as unknown as AuthContextValue;
    const [router] = useState(() => {
        const created = createRouter({ routeTree, context: { auth }, history: createMemoryHistory({ initialEntries: ["/settings"] }) });
        onRouter(created as unknown as TestRouter);
        return created;
    });
    return (
        <AuthContext value={auth}>
            <RouterProvider router={router} context={{ auth }} />
        </AuthContext>
    );
}

function renderPage() {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    let router: TestRouter | null = null;
    render(
        <QueryClientProvider client={queryClient}>
            <AppearanceProvider>
                <Harness onRouter={(created) => (router = created)} />
            </AppearanceProvider>
        </QueryClientProvider>,
    );
    return { router: () => router as unknown as TestRouter };
}

async function choose(label: string, option: string) {
    const user = userEvent.setup();
    await user.click(screen.getByRole("combobox", { name: label }));
    await user.click(await screen.findByRole("option", { name: option }));
}

const row = (label: string) => screen.getByText(label, { selector: "label" }).closest("[class*='px-5']") as HTMLElement;

describe("settings form", () => {
    const baseline = draftFrom({ ...DEFAULT_APP_SETTINGS, terminalShell: "/usr/bin/fish" }, "https://controller.example/", 7);

    it("reads a stored custom shell and compares URLs by origin", () => {
        expect(baseline).toMatchObject({ shell: "custom", customShell: "/usr/bin/fish", fontSize: "13", scrollback: 1_000, refreshInterval: 5_000 });
        expect(changedRows(baseline, { ...baseline, agentUrl: "https://controller.example" })).toEqual([]);
        expect(changedRows(baseline, { ...baseline, fontSize: "13 " })).toEqual([]);
        expect(changedRows(baseline, { ...baseline, shell: "auto", retentionDays: 30 })).toEqual(["shell", "retentionDays"]);
    });

    it("keeps offering a stored value the lists no longer have", () => {
        expect(withCurrent(REFRESH_INTERVALS, 3_000)).toEqual([2_000, 3_000, 5_000, 10_000, 30_000, 60_000]);
        expect(withCurrent(REFRESH_INTERVALS, 5_000)).toBe(REFRESH_INTERVALS);
        expect(draftFrom({ ...DEFAULT_APP_SETTINGS, refreshInterval: null }, "https://controller.example/", null).refreshInterval).toBe(5_000);
    });

    it("explains each invalid field", () => {
        expect(fieldErrors(baseline)).toEqual({});
        const errors = fieldErrors({ ...baseline, agentUrl: "http://agent.example.com", customShell: "fish", fontSize: "30" });
        expect(errors.agentUrl).toBe("Use https:// (http:// only for localhost).");
        expect(errors.shell).toMatch(/absolute path/);
        expect(errors.fontSize).toBe("Use a size from 10 to 24.");
        expect(fieldErrors({ ...baseline, agentUrl: "http://127.0.0.1:5150" }).agentUrl).toBeUndefined();
        expect(fieldErrors({ ...baseline, shell: "custom", customShell: "/bin/my shell" }).shell).toBeDefined();
    });

    it("parses the controller status exactly and refuses anything else", () => {
        expect(controllerStatusFromIpc(nodes.status)).toEqual(nodes.status);
        expect(() => controllerStatusFromIpc({ ...nodes.status, secret: "canary" })).toThrow();
        expect(() => controllerStatusFromIpc({ ...nodes.status, logLevel: "info\n" })).toThrow();
        expect(() => controllerStatusFromIpc({ ...nodes.status, startedAt: "yesterday" })).toThrow();
    });
});

describe("Settings page", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        document.documentElement.classList.remove("dark");
        mockCommands();
    });

    it("shows every section with the stored and controller values", async () => {
        renderPage();
        expect(await screen.findByRole("heading", { name: "SETTINGS" })).toBeInTheDocument();
        for (const section of ["Application", "Agent", "Integrations"]) expect(screen.getByRole("region", { name: section })).toBeInTheDocument();
        expect(screen.getByRole("combobox", { name: "Theme" })).toHaveTextContent("Dark");
        expect(screen.getByRole("combobox", { name: "Dashboard Refresh Interval" })).toHaveTextContent("5s");
        expect(screen.getByRole("combobox", { name: "Default Terminal Shell" })).toHaveTextContent("auto");
        expect(screen.getByRole("textbox", { name: "Terminal Font Size" })).toHaveValue("14");
        expect(screen.getByRole("combobox", { name: "Terminal Scrollback Buffer" })).toHaveTextContent("10,000");
        expect(screen.getByRole("textbox", { name: "Agent URL" })).toHaveValue(session.origin);
        expect(screen.getByRole("textbox", { name: "Auth Provider" })).toHaveValue("Password");
        expect(await screen.findByRole("combobox", { name: "Metrics Retention" })).toHaveTextContent("7 days");
        await waitFor(() => expect(screen.getByRole("textbox", { name: "Log Level" })).toHaveValue("info"));
        expect(within(screen.getByRole("region", { name: "Agent" })).getByText("v0.1.0")).toBeInTheDocument();
        expect(screen.getByRole("textbox", { name: "Signed in as" })).toHaveValue("Owner · owner@example.com");
        expect(screen.getByRole("textbox", { name: "Uptime" })).not.toHaveValue("—");
        expect(screen.getByRole("button", { name: "Integrations hub" })).not.toHaveAttribute("data-restricted");
        expect(screen.queryByRole("region", { name: "Pending changes" })).not.toBeInTheDocument();
        expect(screen.getByText("AGENT v0.1.0")).toBeInTheDocument();
    });

    it("opens the integrations hub, then GitHub, from the Integrations row", async () => {
        const { router } = renderPage();
        const user = userEvent.setup();
        await user.click(await screen.findByRole("button", { name: "Integrations hub" }));
        await waitFor(() => expect(router().state.location.pathname).toBe("/settings/integrations"));
        const breadcrumb = () => within(screen.getByRole("navigation", { name: "Breadcrumb" }));
        expect(breadcrumb().getByRole("link", { name: "Settings" })).toHaveAttribute("href", "/settings");
        await user.click(screen.getByRole("link", { name: "GitHub (not connected)" }));
        expect(await screen.findByRole("heading", { name: "GITHUB INTEGRATION" })).toBeInTheDocument();
        expect(breadcrumb().getByRole("link", { name: "Integrations" })).toHaveAttribute("href", "/settings/integrations");
    });

    it("stages edits with counts and notes, validates on blur, and discards everything", async () => {
        renderPage();
        await screen.findByRole("combobox", { name: "Metrics Retention" });
        await choose("Dashboard Refresh Interval", "10s");
        expect(within(row("Dashboard Refresh Interval")).getByText("Changed from 5s")).toBeInTheDocument();
        const bar = screen.getByRole("region", { name: "Pending changes" });
        expect(bar).toHaveTextContent("1 unsaved change");

        const user = userEvent.setup();
        const url = screen.getByRole("textbox", { name: "Agent URL" });
        await user.clear(url);
        await user.type(url, "http://agent.example.com");
        expect(screen.queryByText("Use https:// (http:// only for localhost).")).not.toBeInTheDocument();
        await user.tab();
        expect(screen.getByText("Use https:// (http:// only for localhost).")).toBeInTheDocument();
        expect(bar).toHaveTextContent("2 unsaved changes · 1 error");
        expect(bar).toHaveTextContent("Fix agent url to save.");
        expect(within(bar).getByRole("button", { name: /SAVE CHANGES/ })).toBeDisabled();

        await user.click(screen.getByRole("button", { name: "Larger" }));
        expect(screen.getByRole("textbox", { name: "Terminal Font Size" })).toHaveValue("15");
        await choose("Default Terminal Shell", "custom");
        const custom = screen.getByRole("textbox", { name: "Custom shell path" });
        await user.type(custom, "fish");
        await user.tab();
        expect(screen.getByText(/Use an absolute path/)).toBeInTheDocument();
        expect(bar).toHaveTextContent("4 unsaved changes · 2 errors");

        await user.click(within(bar).getByRole("button", { name: /DISCARD/ }));
        expect(screen.queryByRole("region", { name: "Pending changes" })).not.toBeInTheDocument();
        expect(screen.getByRole("textbox", { name: "Agent URL" })).toHaveValue(session.origin);
        expect(screen.getByRole("combobox", { name: "Dashboard Refresh Interval" })).toHaveTextContent("5s");
        expect(invoke).not.toHaveBeenCalledWith("save_app_settings", expect.anything());
    });

    it("saves device settings and the retention together, applies the theme, and can undo", async () => {
        renderPage();
        await screen.findByRole("combobox", { name: "Metrics Retention" });
        await waitFor(() => expect(document.documentElement).toHaveClass("dark"));
        await choose("Theme", "Light");
        expect(document.documentElement).toHaveClass("dark");
        await choose("Metrics Retention", "30 days");
        await userEvent.setup().click(screen.getByRole("button", { name: /SAVE CHANGES/ }));

        await waitFor(() => expect(invoke).toHaveBeenCalledWith("update_settings", expect.objectContaining({ metricsRetentionDays: 30 })));
        expect(invoke).toHaveBeenCalledWith("save_app_settings", {
            settings: expect.objectContaining({ scheme: "light", refreshInterval: 5_000, terminalFontSize: 14 }),
        });
        await waitFor(() => expect(document.documentElement).not.toHaveClass("dark"));
        expect(screen.queryByRole("region", { name: "Pending changes" })).not.toBeInTheDocument();
        expect(screen.getByRole("combobox", { name: "Metrics Retention" })).toHaveTextContent("30 days");
        expect(toasts.success).toHaveBeenCalledWith("Settings saved", expect.objectContaining({ description: "2 changes saved." }));

        const [, options] = toasts.success.mock.calls[0] as [string, { action: { onClick: () => void } }];
        await act(async () => options.action.onClick());
        await waitFor(() => expect(retention).toBe(7));
        expect(stored.scheme).toBe("dark");
        await waitFor(() => expect(document.documentElement).toHaveClass("dark"));
    });

    it("asks before leaving with unsaved changes, and stays or leaves", async () => {
        const { router } = renderPage();
        await screen.findByRole("combobox", { name: "Metrics Retention" });
        await choose("Terminal Scrollback Buffer", "50,000");
        const user = userEvent.setup();

        await user.click(screen.getByRole("link", { name: "Logs" }));
        const dialog = await screen.findByRole("alertdialog");
        expect(dialog).toHaveTextContent("Leave with unsaved changes?");
        expect(dialog).toHaveTextContent("1 unsaved change on this page will be lost.");
        await user.click(within(dialog).getByRole("button", { name: "CANCEL" }));
        expect(router().state.location.pathname).toBe("/settings");
        expect(screen.getByRole("region", { name: "Pending changes" })).toBeInTheDocument();

        await user.click(screen.getByRole("link", { name: "Logs" }));
        await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: /LEAVE/ }));
        await waitFor(() => expect(router().state.location.pathname).toBe("/logs"));
    });

    it("confirms an agent URL change, then signs out and opens Connect with the new origin", async () => {
        const { router } = renderPage();
        await screen.findByRole("combobox", { name: "Metrics Retention" });
        const user = userEvent.setup();
        await choose("Dashboard Refresh Interval", "30s");
        const url = screen.getByRole("textbox", { name: "Agent URL" });
        await user.clear(url);
        await user.type(url, "https://agent-eu.example.com");
        await user.click(screen.getByRole("button", { name: /SAVE CHANGES/ }));

        const dialog = await screen.findByRole("alertdialog");
        expect(dialog).toHaveTextContent("Switch to https://agent-eu.example.com?");
        expect(dialog).toHaveTextContent("You'll be signed out of Production");
        await user.click(within(dialog).getByRole("button", { name: "CANCEL" }));
        expect(logout).not.toHaveBeenCalled();
        expect(screen.getByRole("textbox", { name: "Agent URL" })).toHaveValue("https://agent-eu.example.com");
        expect(screen.getByRole("region", { name: "Pending changes" })).toHaveTextContent("2 unsaved changes");

        await user.click(screen.getByRole("button", { name: /SAVE CHANGES/ }));
        await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: /SWITCH & SIGN IN/ }));
        await waitFor(() => expect(logout).toHaveBeenCalled());
        expect(invoke).toHaveBeenCalledWith("save_app_settings", { settings: expect.objectContaining({ refreshInterval: 30_000 }) });
        await waitFor(() => expect(router().state.location.pathname).toBe("/login"));
        await waitFor(() => expect(screen.getByDisplayValue("https://agent-eu.example.com")).toBeInTheDocument());
        expect(router().state.location.search).toMatchObject({ origin: "https://agent-eu.example.com" });
    });
});
