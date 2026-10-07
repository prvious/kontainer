import { useState } from "react";
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRouter, RouterProvider } from "@tanstack/react-router";
import { routeTree } from "@/routeTree.gen";
import { AuthContext, type AuthContextValue } from "@/contexts/auth-context-value";
import services from "../fixtures/services-ipc.json";

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
const auth = {
    session,
    isLoading: false,
    expired: false,
    issue: null,
    reportInventoryCustodyError: vi.fn(),
    logout: vi.fn(),
    retryCommit: vi.fn(),
    retryRestore: vi.fn(),
} as unknown as AuthContextValue;

const [web] = services.serviceList.items;
const STREAM = "018f2e34-0000-7000-8000-00000000f001";

type Channel = { onmessage: (message: unknown) => void };
let channels: Channel[] = [];
let serviceList = services.serviceList;

function mockCommands() {
    channels = [];
    serviceList = services.serviceList;
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
        switch (command) {
            case "logs_open":
                channels.push((args as { channel: Channel }).channel);
                return `018f2e34-0000-7000-8000-00000000f00${channels.length}`;
            case "logs_close":
            case "logs_retry_now":
                return null;
            case "list_services":
                return serviceList;
            case "load_app_settings":
                return { refreshInterval: 60_000 };
            case "controller_health":
                return { status: "ready" };
            case "session_connections":
                return [];
            default:
                return null;
        }
    });
}

function Harness({ path }: { path: string }) {
    const [router] = useState(() => createRouter({ routeTree, context: { auth }, history: createMemoryHistory({ initialEntries: [path] }) }));
    return <RouterProvider router={router} context={{ auth }} />;
}

function renderPage(path = `/logs?service=${web.name}`) {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    return render(
        <AuthContext value={auth}>
            <QueryClientProvider client={queryClient}>
                <Harness path={path} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

let ts = 1_790_000_000_000;
function lines(...texts: string[]) {
    return {
        type: "lines",
        lines: texts.map((text, index) => {
            ts += 10;
            return {
                seq: index + 1,
                ts,
                stream: text.startsWith("!") ? "stderr" : "stdout",
                taskId: "t",
                nodeId: "n",
                text: text.replace(/^!/, ""),
                truncated: false,
            };
        }),
    };
}

async function push(event: unknown, channel = 0) {
    await act(async () => channels[channel].onmessage(event));
}

const originalHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetHeight");

describe("Logs page", () => {
    beforeAll(() => {
        // jsdom lays nothing out; the virtualizer needs a viewport to render rows into, and the
        // picker's list scrolls its highlighted item into view.
        Object.defineProperty(HTMLElement.prototype, "offsetHeight", { configurable: true, get: () => 600 });
        Element.prototype.scrollIntoView = vi.fn();
    });
    afterAll(() => {
        if (originalHeight) Object.defineProperty(HTMLElement.prototype, "offsetHeight", originalHeight);
        delete (Element.prototype as { scrollIntoView?: unknown }).scrollIntoView;
    });
    beforeEach(() => {
        vi.clearAllMocks();
        mockCommands();
    });

    it("opens a tab from the search param and shows lines with their levels", async () => {
        renderPage();
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith(
                "logs_open",
                expect.objectContaining({ profileId: session.profileId, serviceId: web.id, since: null, tail: 500 }),
            ),
        );
        expect(await screen.findByRole("tab", { name: new RegExp(web.name) })).toHaveAttribute("aria-selected", "true");
        await push(lines("level=info started", "WARN slow", "!panic: boom", "GET / 200"));
        const view = screen.getByRole("table", { name: "Log lines" });
        expect(within(view).getByRole("rowgroup")).toHaveAttribute("tabindex", "0");
        expect(within(view).getByText("level=info started")).toBeInTheDocument();
        expect(within(view).getByText("INFO")).toBeInTheDocument();
        expect(within(view).getByText("WARN")).toBeInTheDocument();
        expect(within(view).getByText("ERR")).toBeInTheDocument();
        expect(within(within(view).getByRole("rowgroup")).getAllByRole("row")).toHaveLength(4);
        await waitFor(() => expect(screen.getByText(/1 LOG STREAMS/)).toBeInTheDocument());
    });

    it("pauses following on scroll up, counts new lines, and jumps back", async () => {
        renderPage();
        await waitFor(() => expect(channels).toHaveLength(1));
        await push(lines("one", "two"));
        const view = within(screen.getByRole("table", { name: "Log lines" })).getByRole("rowgroup");
        Object.defineProperties(view, {
            scrollHeight: { configurable: true, value: 2_000 },
            clientHeight: { configurable: true, value: 600 },
            scrollTop: { configurable: true, writable: true, value: 100 },
        });
        fireEvent.scroll(view);
        await push(lines("three", "four", "five"));
        expect(await screen.findByText("3 new lines")).toBeInTheDocument();
        await userEvent.setup().click(screen.getByRole("button", { name: "JUMP TO LATEST" }));
        expect(screen.queryByText(/new lines/)).not.toBeInTheDocument();
    });

    it("keeps lines while reconnecting, counts down, retries now, and marks dropped lines", async () => {
        renderPage();
        await waitFor(() => expect(channels).toHaveLength(1));
        await push(lines("before the drop"));
        await push({ type: "reconnecting", attempt: 3, nextRetryMs: 4_000 });
        expect(await screen.findByText(`Reconnecting to ${web.name}… attempt 3 · next try in 4s`)).toBeInTheDocument();
        expect(screen.getByText("before the drop")).toBeInTheDocument();
        expect(screen.getByTestId(`log-tab-dot-${web.name}`)).toHaveAttribute("data-tone", "warning");
        await userEvent.setup().click(screen.getByRole("button", { name: /RETRY NOW/ }));
        expect(invoke).toHaveBeenCalledWith("logs_retry_now", { streamId: STREAM });
        await push({ type: "reconnected" });
        await push({ type: "gap", dropped: 214 });
        await push(lines("after the drop"));
        expect(screen.queryByText(/Reconnecting to/)).not.toBeInTheDocument();
        expect(screen.getByText("214 lines dropped while the client caught up")).toBeInTheDocument();
        expect(screen.getByTestId(`log-tab-dot-${web.name}`)).toHaveAttribute("data-tone", "success");
    });

    it("explains a filter with no matches and loads the previous hour", async () => {
        renderPage();
        await waitFor(() => expect(channels).toHaveLength(1));
        await push(lines("GET / 200", "GET /health 200"));
        const user = userEvent.setup();
        await user.type(screen.getByRole("searchbox", { name: "Filter logs" }), "timeout");
        expect(await screen.findByText("No lines match 'timeout'")).toBeInTheDocument();
        expect(screen.getByText(new RegExp(`Searched the last 2 lines of ${web.name} since \\d\\d:\\d\\d`))).toBeInTheDocument();
        expect(screen.getByText("0 matches")).toBeInTheDocument();

        const before = Date.now();
        await user.click(screen.getByRole("button", { name: /LOAD PREVIOUS HOUR/ }));
        await waitFor(() => expect(channels).toHaveLength(2));
        const reopen = vi.mocked(invoke).mock.calls.filter(([command]) => command === "logs_open")[1][1] as { since: number; tail: number };
        expect(reopen.tail).toBe(5_000);
        expect(reopen.since).toBeGreaterThanOrEqual(before - 3_600_000);
        expect(reopen.since).toBeLessThanOrEqual(Date.now() - 3_600_000);
        expect(invoke).toHaveBeenCalledWith("logs_close", { streamId: STREAM });

        await user.click(screen.getByRole("button", { name: /CLEAR FILTER/ }));
        expect(screen.getByRole("searchbox", { name: "Filter logs" })).toHaveValue("");
        await user.type(screen.getByRole("searchbox", { name: "Filter logs" }), "/health/");
        // The replaced stream's late lines don't reach the new buffer.
        await push(lines("GET /health 200 from the old stream"), 0);
        await push(lines("GET /health 200"), 1);
        expect(await screen.findByText("1 match")).toBeInTheDocument();
        expect(screen.queryByText("GET /health 200 from the old stream")).not.toBeInTheDocument();
    });

    it("keeps a tab whose service was removed, explains it, and resumes from the last line", async () => {
        renderPage();
        await waitFor(() => expect(channels).toHaveLength(1));
        await push(lines("last words"));
        const lastTs = ts;
        await push({ type: "ended", reason: "service_removed", code: "service_removed", message: "The service was removed." });
        expect(await screen.findByText(`${web.name} was removed — the stream stopped`)).toBeInTheDocument();
        expect(screen.getByText("last words")).toBeInTheDocument();
        expect(screen.getByTestId(`log-tab-dot-${web.name}`)).toHaveAttribute("data-tone", "destructive");
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: /RESUME/ }));
        await waitFor(() => expect(channels).toHaveLength(2));
        expect(invoke).toHaveBeenCalledWith("logs_open", expect.objectContaining({ serviceId: web.id, since: lastTs, tail: 5_000 }));
        expect(screen.getByText("last words")).toBeInTheDocument();

        await user.click(screen.getByRole("button", { name: `Close ${web.name}` }));
        expect(await screen.findByText("No log streams open")).toBeInTheDocument();
        expect(invoke).toHaveBeenCalledWith("logs_close", { streamId: "018f2e34-0000-7000-8000-00000000f002" });
    });

    it("resumes a redeployed service under its new id, by name", async () => {
        renderPage();
        await waitFor(() => expect(channels).toHaveLength(1));
        await push(lines("before the redeploy"));
        await push({ type: "ended", reason: "service_removed", code: "service_removed", message: "The service was removed." });
        // Redeployed: same name, new id; the services cache still holds the old one.
        const redeployed = "webappweb1111111111111111";
        serviceList = {
            ...services.serviceList,
            items: services.serviceList.items.map((service) => (service.id === web.id ? { ...service, id: redeployed } : service)),
        };
        await userEvent.setup().click(await screen.findByRole("button", { name: /RESUME/ }));
        await waitFor(() => expect(channels).toHaveLength(2));
        const opens = vi.mocked(invoke).mock.calls.filter(([command]) => command === "logs_open");
        expect(opens[1][1]).toMatchObject({ serviceId: redeployed });
        expect(screen.getByRole("tab", { name: new RegExp(web.name) })).toBeInTheDocument();
        expect(screen.getByText("before the redeploy")).toBeInTheDocument();
    });

    it("closes the focused tab with Delete; its close button stays out of the tab order", async () => {
        renderPage();
        const tab = await screen.findByRole("tab", { name: new RegExp(web.name) });
        await waitFor(() => expect(screen.getByTestId(`log-tab-dot-${web.name}`)).toHaveAttribute("data-tone", "success"));
        expect(tab).toHaveAttribute("aria-keyshortcuts", "Delete");
        expect(screen.getByRole("button", { name: `Close ${web.name}` })).toHaveAttribute("tabindex", "-1");
        act(() => tab.focus());
        await userEvent.setup().keyboard("{Delete}");
        expect(await screen.findByText("No log streams open")).toBeInTheDocument();
        expect(invoke).toHaveBeenCalledWith("logs_close", { streamId: STREAM });
    });

    it("opens more streams from the picker, side by side", async () => {
        renderPage("/logs");
        const user = userEvent.setup();
        await screen.findByText("No log streams open");
        await user.click(screen.getAllByRole("button", { name: /NEW STREAM/ })[0]);
        await user.click(await screen.findByRole("option", { name: new RegExp(web.name) }));
        await waitFor(() => expect(channels).toHaveLength(1));
        const other = services.serviceList.items[1];
        await user.click(screen.getByRole("button", { name: "New stream" }));
        await user.click(await screen.findByRole("option", { name: new RegExp(other.name) }));
        await waitFor(() => expect(channels).toHaveLength(2));
        expect(screen.getAllByRole("tab")).toHaveLength(2);
        expect(screen.getByRole("tab", { name: new RegExp(other.name) })).toHaveAttribute("aria-selected", "true");

        // A service that already has a tab gets it focused, not a second stream.
        await user.click(screen.getByRole("button", { name: "New stream" }));
        await user.click(await screen.findByRole("option", { name: new RegExp(web.name) }));
        expect(screen.getByRole("tab", { name: new RegExp(web.name) })).toHaveAttribute("aria-selected", "true");
        expect(screen.getAllByRole("tab")).toHaveLength(2);
        expect(channels).toHaveLength(2);
    });
});
