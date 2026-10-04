import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRouter, RouterProvider } from "@tanstack/react-router";
import { routeTree } from "@/routeTree.gen";
import { AuthContext, type AuthContextValue } from "@/contexts/auth-context-value";
import nodes from "../fixtures/nodes-ipc.json";
import containers from "../fixtures/containers-ipc.json";

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

/** The last hour of cluster CPU with a 62 % peak and one empty bucket. */
const hour = {
    ...nodes.clusterSamples,
    window: "1h",
    bucket: "60s",
    points: [
        { ...nodes.clusterSamples.points[0], at: "2026-09-28T11:58:00.000Z" },
        { ...nodes.clusterSamples.points[1], at: "2026-09-28T11:59:00.000Z", cpuPercent: 62 },
        { ...nodes.clusterSamples.points[1], at: "2026-09-28T12:00:00.000Z", cpuPercent: 30 },
    ],
};

function manyContainers(count: number) {
    const [template] = containers.containerList.items;
    return {
        ...containers.containerList,
        items: Array.from({ length: count }, (_, index) => ({
            ...template,
            containerId: index.toString(16).padStart(64, "0"),
            name: `web.${index}`,
            taskId: null,
            serviceName: null,
            stackNamespace: null,
            cpuPercent: index,
        })),
    };
}

type Handler = (args: Record<string, unknown>) => unknown;

function mockCommands(overrides: Record<string, Handler> = {}) {
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
        if (command in overrides) return overrides[command]((args ?? {}) as Record<string, unknown>);
        switch (command) {
            case "list_nodes":
                return nodes.nodeList;
            case "get_cluster_summary":
                return nodes.clusterSummary;
            case "get_cluster_samples":
                return hour;
            case "list_containers":
                return containers.containerList;
            case "controller_health":
                return { status: "ready" };
            case "load_app_settings":
                return { refreshInterval: 60_000 };
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

function renderPage(path = "/") {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    return render(
        <AuthContext value={auth}>
            <QueryClientProvider client={queryClient}>
                <Harness path={path} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

const card = (name: string) => screen.getByRole("region", { name });

describe("Overview page", () => {
    beforeEach(() => vi.clearAllMocks());
    afterEach(() => vi.useRealTimers());

    it("fills the four cards, the Traefik skeleton, and the busiest containers first", async () => {
        mockCommands();
        renderPage();
        await screen.findByRole("region", { name: "CONTAINERS" });
        expect(screen.getByText(/^for Production · 3 nodes · swarm active · synced \d+s ago$/)).toBeInTheDocument();

        expect(within(card("CONTAINERS")).getByText("/ 2")).toBeInTheDocument();
        expect(within(card("CONTAINERS")).getByText(/1 RUNNING/)).toBeInTheDocument();
        expect(within(card("CONTAINERS")).getByText(/1 EXITED/)).toBeInTheDocument();
        expect(within(card("CPU USAGE")).getByText("13")).toBeInTheDocument();
        await within(card("CPU USAGE")).findByText("across 1 node · peak 62%");
        expect(within(card("MEMORY")).getByText("GB / 8 GB")).toBeInTheDocument();
        expect(within(card("MEMORY")).getByText("13% allocated · 7 GB free")).toBeInTheDocument();
        expect(within(card("NETWORK I/O")).getByText("KB/s")).toBeInTheDocument();
        expect(within(card("NETWORK I/O")).getByText("up 0 B/s · down 2 KB/s")).toBeInTheDocument();
        expect(screen.getByRole("region", { name: "Traefik (not connected)" })).toBeInTheDocument();

        expect(screen.getByText("RUNNING CONTAINERS")).toBeInTheDocument();
        expect(screen.queryByLabelText("Select all rows")).not.toBeInTheDocument();
        const names = screen.getAllByText(/^(shop_web\.1\.x7kq|scratch)$/).map((element) => element.textContent);
        expect(names).toEqual(["shop_web.1.x7kq", "scratch"]);
        expect(screen.getByRole("button", { name: /DEPLOY/ })).toHaveAttribute("aria-disabled", "true");
    });

    it("shows skeletons only on first load and spins REFRESH on a refetch", async () => {
        let release: (value: unknown) => void = () => {};
        let calls = 0;
        mockCommands({
            list_containers: () => {
                calls += 1;
                return calls === 1 ? containers.containerList : new Promise((resolve) => (release = resolve));
            },
            get_cluster_summary: () => new Promise((resolve) => (release = resolve)),
        });
        renderPage();
        await waitFor(() => expect(screen.getAllByTestId("skeleton-stat")).toHaveLength(4));
        expect(screen.getByText("syncing cluster state…")).toBeInTheDocument();
        await act(async () => release(nodes.clusterSummary));
        await screen.findByRole("region", { name: "CPU USAGE" });
        expect(screen.queryByTestId("skeleton-stat")).not.toBeInTheDocument();

        await userEvent.setup().click(screen.getByRole("button", { name: /REFRESH/ }));
        await waitFor(() => expect(screen.getByRole("button", { name: /REFRESH/ })).toHaveAttribute("aria-busy", "true"));
        expect(screen.getByRole("region", { name: "CPU USAGE" })).toBeInTheDocument();
        expect(screen.queryByTestId("skeleton-stat")).not.toBeInTheDocument();
        await act(async () => release(containers.containerList));
    });

    it("shows an error, not skeletons forever, when the summary fails before its first answer", async () => {
        let failing = true;
        mockCommands({
            get_cluster_summary: () => {
                if (failing) throw "controller_unavailable";
                return nodes.clusterSummary;
            },
        });
        renderPage();
        expect(await screen.findByText("Couldn't load cluster figures")).toBeInTheDocument();
        expect(screen.queryByTestId("skeleton-stat")).not.toBeInTheDocument();
        expect(screen.getByText("couldn't sync cluster state")).toBeInTheDocument();
        expect(screen.getByText("controller_unavailable")).toBeInTheDocument();

        failing = false;
        await userEvent.setup().click(screen.getByRole("button", { name: /TRY AGAIN/ }));
        expect(await screen.findByRole("region", { name: "CPU USAGE" })).toBeInTheDocument();
    });

    it("keeps cached figures dimmed and pauses changes after three failed probes", async () => {
        mockCommands({ controller_health: () => ({ status: "unreachable", diagnostic: "tcp" }) });
        renderPage();
        await screen.findByRole("region", { name: "CPU USAGE" });
        expect(screen.getByRole("button", { name: "Pause" })).not.toHaveAttribute("aria-disabled");

        const alert = await screen.findByText("Showing cached data — controller unreachable", {}, { timeout: 4000 });
        expect(alert.closest("[data-tone]")).toHaveTextContent(
            /Last successful sync \d\d:\d\d:\d\d \(just now\)\. Changes are paused until the connection returns\./,
        );
        expect(screen.getAllByText(/^as of \d\d:\d\d$/)).toHaveLength(4);
        expect(screen.getByText(/^RUNNING CONTAINERS · AS OF \d\d:\d\d$/)).toBeInTheDocument();
        expect(screen.getByText(/stale — last synced \d\d:\d\d:\d\d/)).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Pause" })).toHaveAttribute("aria-disabled", "true");
        expect(screen.getByText("RECONNECTING")).toBeInTheDocument();
    });

    it("says it reconnected for five seconds once a probe answers again", async () => {
        vi.useFakeTimers({ shouldAdvanceTime: true });
        let reachable = false;
        mockCommands({ controller_health: () => (reachable ? { status: "ready" } : { status: "unreachable", diagnostic: "tcp" }) });
        renderPage();
        await screen.findByRole("region", { name: "CPU USAGE" });
        await act(() => vi.advanceTimersByTimeAsync(2_500));
        const retry = await screen.findByRole("button", { name: /RETRY NOW/ });

        reachable = true;
        await userEvent.setup({ advanceTimers: vi.advanceTimersByTime }).click(retry);
        expect(await screen.findByText("Reconnected — the controller is reachable again")).toBeInTheDocument();
        expect(screen.queryByText("Showing cached data — controller unreachable")).not.toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Pause" })).not.toHaveAttribute("aria-disabled");

        await act(() => vi.advanceTimersByTimeAsync(5_500));
        expect(screen.queryByText("Reconnected — the controller is reachable again")).not.toBeInTheDocument();
    });

    it("breaks CPU down per node on hover and calls silent nodes unreachable, not 0%", async () => {
        mockCommands();
        renderPage();
        const ring = await screen.findByRole("img", { name: "Aggregate CPU by node" });
        await userEvent.setup().hover(ring);
        expect(await screen.findByText("CPU by node · 4 + 2 + 2 cores")).toBeInTheDocument();
        expect(screen.getAllByText("unreachable")).toHaveLength(2);
        expect(screen.getByText("13%")).toBeInTheDocument();
        expect(screen.queryByText("0%")).not.toBeInTheDocument();
    });

    it("caps the table at the 50 busiest and links VIEW ALL to the Containers page", async () => {
        mockCommands({ list_containers: () => manyContainers(60) });
        renderPage();
        await screen.findByText("web.59");
        expect(screen.getAllByText(/^web\.\d+$/)).toHaveLength(50);
        expect(screen.queryByText("web.9")).not.toBeInTheDocument();
        expect(screen.getByText("50 OF 60")).toBeInTheDocument();
        expect(screen.getByRole("link", { name: "VIEW ALL" })).toHaveAttribute("href", "/containers");
    });
});
