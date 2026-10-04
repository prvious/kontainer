import { useState } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRouter, RouterProvider } from "@tanstack/react-router";
import { routeTree } from "@/routeTree.gen";
import { AuthContext, type AuthContextValue } from "@/contexts/auth-context-value";
import nodes from "../fixtures/nodes-ipc.json";

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

const ago = (ms: number) => new Date(Date.now() - ms).toISOString();
const [manager, worker2, worker3] = nodes.nodeList.items;

/** Manager active, worker-1 with a stale agent, worker-2 down for 47 minutes, worker-3 without an agent. */
function nodeList() {
    const worker1 = {
        ...manager,
        id: "worker-1",
        hostname: "worker-1",
        isLeader: false,
        role: "worker",
        agentStatus: "degraded",
        health: "degraded",
        lastCheckInAt: ago(42_000),
    };
    return { ...nodes.nodeList, items: [manager, worker1, { ...worker2, lastCheckInAt: ago(47 * 60_000) }, worker3] };
}

const summary = { ...nodes.clusterSummary, nodesTotal: 4, nodesDegraded: 2 };

/**
 * Twelve whole 10 s buckets, then the one still filling (99 %, one node so far). The newest whole
 * bucket has two of four nodes reporting.
 */
function series() {
    const end = Math.floor(Date.now() / 10_000) * 10_000;
    const points = Array.from({ length: 13 }, (_, index) => ({
        at: new Date(end - (12 - index) * 10_000).toISOString(),
        cpuPercent: index === 12 ? 99 : 20 + index,
        memoryUsedBytes: 2 * 1024 ** 3,
        diskUsedBytes: null,
        containersRunning: 5,
        netRxBytesPerSec: null,
        netTxBytesPerSec: null,
        nodes: index === 12 ? 1 : index === 11 ? 2 : 4,
    }));
    return { window: "5m", bucket: "10s", points, snapshotAt: new Date(end).toISOString() };
}

function mockCommands(overrides: Record<string, (args: Record<string, unknown>) => unknown> = {}) {
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
        if (command in overrides) return overrides[command]((args ?? {}) as Record<string, unknown>);
        switch (command) {
            case "list_nodes":
                return nodeList();
            case "get_cluster_summary":
                return summary;
            case "get_cluster_samples":
                return series();
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

function renderPage() {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
        <AuthContext value={auth}>
            <QueryClientProvider client={queryClient}>
                <Harness path='/nodes' />
            </QueryClientProvider>
        </AuthContext>,
    );
    return queryClient;
}

describe("Nodes page", () => {
    beforeEach(() => vi.clearAllMocks());

    it("renders one card per node with role, health, and the degraded and down states", async () => {
        mockCommands();
        renderPage();
        const leader = await screen.findByRole("article", { name: "manager" });
        expect(screen.getByText("4 nodes · 1 active · 2 degraded · 1 down · Docker Swarm cluster")).toBeInTheDocument();
        expect(within(leader).getByText("LEADER")).toBeInTheDocument();
        expect(within(leader).getByText("active")).toBeInTheDocument();
        expect(within(leader).getByText("10.0.1.10")).toBeInTheDocument();
        expect(within(leader).getByText("Alpine Linux v3.22/aarch64")).toBeInTheDocument();
        expect(within(leader).getByText("5 running")).toBeInTheDocument();
        expect(within(leader).getByText("13% · 4 cores")).toBeInTheDocument();
        expect(within(leader).getByText("1 GB / 8 GB")).toBeInTheDocument();
        expect(within(leader).queryByRole("status")).not.toBeInTheDocument();

        const stale = screen.getByRole("article", { name: "worker-1" });
        expect(within(stale).getByText("WORKER")).toBeInTheDocument();
        expect(within(stale).getByRole("status")).toHaveTextContent(/^Agent last checked in 4[1-3]s ago$/);
        expect(within(stale).getByText("5 running")).toBeInTheDocument();

        const down = screen.getByRole("article", { name: "worker-2" });
        expect(within(down).getByText("UNKNOWN")).toBeInTheDocument();
        expect(within(down).getByRole("alert")).toHaveTextContent("Node unreachable since 47m ago");
        expect(within(down).getByText("— · 2 cores")).toBeInTheDocument();
        expect(within(down).getByText("— / 4 GB")).toBeInTheDocument();
        for (const bar of within(down).getAllByRole("progressbar")) expect(bar).not.toHaveAttribute("aria-valuenow");

        const agentless = screen.getByRole("article", { name: "worker-3" });
        expect(within(agentless).getByText("DRAIN")).toBeInTheDocument();
        expect(within(agentless).getByRole("status")).toHaveTextContent("No Kontainer agent has checked in from this node");
    });

    it("fills the cluster load strip over the last 2 minutes and says how many nodes reported in a hovered bucket", async () => {
        mockCommands();
        renderPage();
        const strip = await screen.findByRole("region", { name: "Cluster load" });
        await within(strip).findByText("4 nodes · 3 reachable · 5 containers");
        expect(within(strip).getByText("HEALTHY")).toBeInTheDocument();
        expect(within(strip).getByText("13")).toBeInTheDocument();
        expect(within(strip).getByText("GB / 8 GB")).toBeInTheDocument();
        expect(invoke).toHaveBeenCalledWith("get_cluster_samples", { profileId: session.profileId, window: "5m", bucket: "10s" });
        expect(within(strip).getByText("LAST 2m")).toBeInTheDocument();

        // The newest bar is the newest whole bucket: the one still filling (99 %) isn't drawn.
        const chart = within(strip).getByRole("img", { name: "Aggregate CPU, last 2 minutes" });
        fireEvent.keyDown(chart, { key: "ArrowLeft" });
        fireEvent.keyDown(chart, { key: "ArrowRight" });
        const tooltip = within(strip).getByRole("tooltip");
        expect(tooltip).toHaveTextContent("CPU31%");
        expect(tooltip).toHaveTextContent("Memory2 GB");
        expect(within(tooltip).getByText("2 of 4 nodes reporting")).toHaveClass("text-warning");
        fireEvent.keyDown(chart, { key: "ArrowLeft" });
        expect(within(strip).getByText("4 of 4 nodes reporting")).toHaveClass("text-muted-foreground");
    });

    it("shows a degraded quorum and keeps ADD NODE disabled", async () => {
        mockCommands({ get_cluster_summary: () => ({ ...summary, quorum: "degraded", managersReachable: 0 }) });
        renderPage();
        const strip = await screen.findByRole("region", { name: "Cluster load" });
        expect(await within(strip).findByText("DEGRADED")).toBeInTheDocument();
        expect(screen.getByRole("button", { name: /ADD NODE/ })).toHaveAttribute("aria-disabled", "true");
    });

    it("says so when no node has checked in", async () => {
        mockCommands({
            list_nodes: () => ({ ...nodes.nodeList, items: [] }),
            get_cluster_summary: () => ({ ...summary, nodesTotal: 0, nodesActive: 0, nodesDegraded: 0, nodesDown: 0 }),
        });
        renderPage();
        expect(await screen.findByText("No nodes yet")).toBeInTheDocument();
    });

    it("keeps the last cards faded under a stale banner when a refresh fails", async () => {
        let failing = false;
        mockCommands({
            list_nodes: () => {
                if (failing) throw "controller_unavailable";
                return nodeList();
            },
        });
        const queryClient = renderPage();
        const leader = await screen.findByRole("article", { name: "manager" });
        expect(screen.queryByText(/Couldn't refresh/)).not.toBeInTheDocument();
        failing = true;
        await act(() => queryClient.refetchQueries({ queryKey: ["nodes", session.profileId], exact: true }));
        expect(await screen.findByText(/^Couldn't refresh — showing the snapshot from \d\d:\d\d:\d\d$/)).toBeInTheDocument();
        expect(screen.getByRole("article", { name: "manager" })).toBe(leader);
        expect(leader.parentElement).toHaveClass("opacity-50");
    });
});
