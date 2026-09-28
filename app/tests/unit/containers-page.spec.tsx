import { useState } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRouter, RouterProvider } from "@tanstack/react-router";
import { routeTree } from "@/routeTree.gen";
import { AuthContext, type AuthContextValue } from "@/contexts/auth-context-value";
import containers from "../fixtures/containers-ipc.json";
import nodes from "../fixtures/nodes-ipc.json";

vi.mock("sonner", () => ({
    toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn(), warning: vi.fn(), loading: vi.fn(), dismiss: vi.fn() }),
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

type ContainerList = typeof containers.containerList;
let answers: ContainerList[] = [];

function mockCommands(overrides: Record<string, (args: unknown) => unknown> = {}) {
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
        if (command in overrides) return overrides[command](args);
        switch (command) {
            case "list_containers":
                return answers.length > 1 ? answers.shift() : answers[0];
            case "list_nodes":
                return nodes.nodeList;
            case "get_cluster_samples":
                return { ...nodes.clusterSamples, window: "24h", bucket: "15m" };
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

function renderPage(path = "/containers") {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const view = render(
        <AuthContext value={auth}>
            <QueryClientProvider client={queryClient}>
                <Harness path={path} />
            </QueryClientProvider>
        </AuthContext>,
    );
    return { ...view, queryClient };
}

const rowFor = (name: string) => screen.getByText(name).closest("tr") as HTMLElement;

describe("Containers page", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        answers = [structuredClone(containers.containerList)];
    });

    it("shows every container with its node, figures, and the states strip", async () => {
        answers = [{ ...containers.containerList, nodes: containers.containerList.nodes.map((node) => ({ ...node, status: "ok" })) }];
        mockCommands();
        renderPage();
        expect(await screen.findByText("shop_web.1.x7kq")).toBeInTheDocument();
        expect(screen.getByText("managing 2 containers across 3 nodes")).toBeInTheDocument();
        const web = rowFor("shop_web.1.x7kq");
        expect(within(web).getByText("UP")).toBeInTheDocument();
        expect(within(web).getByText("shop_web · shop")).toBeInTheDocument();
        expect(within(web).getByText("12.5%")).toBeInTheDocument();
        expect(within(web).getByText("50 MB")).toBeInTheDocument();
        expect(within(web).getByText("8080")).toBeInTheDocument();
        const scratch = rowFor("scratch");
        expect(within(scratch).getByText("EXITED")).toBeInTheDocument();
        expect(within(scratch).getAllByText("—").length).toBeGreaterThan(0);
        expect(screen.getByText("1 RUNNING")).toBeInTheDocument();
        expect(screen.getByText("1 EXITED")).toBeInTheDocument();
        expect(invoke).toHaveBeenCalledWith("list_containers", { profileId: session.profileId, nodeId: null });
    });

    it("keeps filters in the URL and clears them from the chips", async () => {
        mockCommands();
        renderPage("/containers?state=exited");
        expect(await screen.findByText("scratch")).toBeInTheDocument();
        expect(screen.queryByText("shop_web.1.x7kq")).not.toBeInTheDocument();
        expect(screen.getByText("status: exited")).toBeInTheDocument();
        await userEvent.setup().click(screen.getByRole("button", { name: "Remove filter status: exited" }));
        expect(await screen.findByText("shop_web.1.x7kq")).toBeInTheDocument();
    });

    it("sorts by name from the URL", async () => {
        mockCommands();
        renderPage("/containers?sort=name");
        await screen.findByText("scratch");
        const names = screen.getAllByRole("row").map((row) => row.textContent ?? "");
        const scratchIndex = names.findIndex((text) => text.includes("scratch"));
        const webIndex = names.findIndex((text) => text.includes("shop_web.1.x7kq"));
        expect(scratchIndex).toBeLessThan(webIndex);
        expect(screen.getByText("Sort: Name")).toBeInTheDocument();
    });

    it("names a silent node and keeps its last rows as UNKNOWN with actions disabled", async () => {
        const silent = structuredClone(containers.containerList);
        silent.items = silent.items.filter((item) => item.nodeId !== "worker-1");
        silent.nodes = silent.nodes.map((node) => (node.nodeId === "worker-1" ? { ...node, status: "timeout" } : node));
        answers = [structuredClone(containers.containerList), silent];
        mockCommands();
        renderPage();
        await screen.findByText("scratch");
        await userEvent.setup().click(screen.getByRole("button", { name: "REFRESH" }));
        expect(await screen.findByText(/worker-1 and worker-2 didn't answer/)).toBeInTheDocument();
        expect(screen.getByText(/1 of 3 nodes answered/)).toBeInTheDocument();
        const scratch = rowFor("scratch");
        expect(within(scratch).getByText("UNKNOWN")).toBeInTheDocument();
        expect(within(scratch).getByRole("button", { name: "Restart" })).toHaveAttribute("aria-disabled", "true");
    });

    it("explains empty filters and offers CLEAR FILTERS and SHOW ALL", async () => {
        mockCommands();
        renderPage("/containers?q=payments&state=running");
        expect(await screen.findByText("No containers match these filters")).toBeInTheDocument();
        expect(screen.getByText(/Nothing matches "payments" with status: running/)).toBeInTheDocument();
        await userEvent.setup().click(screen.getByRole("button", { name: "SHOW ALL 2" }));
        // The search text stays, so nothing matches yet; the chips are gone.
        await waitFor(() => expect(screen.queryByText("status: running")).not.toBeInTheDocument());
        await userEvent.setup().click(screen.getByRole("button", { name: "CLEAR FILTERS" }));
        expect(await screen.findByText("scratch")).toBeInTheDocument();
    });

    it("shows the Foundations empty copy when the cluster has no containers", async () => {
        answers = [{ ...containers.containerList, items: [] }];
        mockCommands();
        renderPage();
        expect(await screen.findByText("No containers yet")).toBeInTheDocument();
        expect(screen.getByRole("button", { name: /DEPLOY STACK/ })).toHaveAttribute("data-restricted", "true");
    });

    it("requires the typed name to remove a running container, not a stopped one", async () => {
        mockCommands({ container_action: () => ({ operationId: "018f2e34-0000-7000-8000-0000000000cc", replayed: false }) });
        renderPage();
        await screen.findByText("scratch");
        const user = userEvent.setup();
        await user.click(within(rowFor("shop_web.1.x7kq")).getByRole("button", { name: /More actions/ }));
        await user.click(await screen.findByRole("menuitem", { name: "Remove" }));
        const dialog = await screen.findByRole("alertdialog");
        const confirm = within(dialog).getByRole("button", { name: /REMOVE/ });
        expect(confirm).toBeDisabled();
        await user.type(within(dialog).getByRole("textbox"), "shop_web.1.x7kq");
        expect(confirm).toBeEnabled();
        await user.click(confirm);
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith(
                "container_action",
                expect.objectContaining({ nodeId: "manager", containerId: "a".repeat(64), action: "remove", options: { force: true } }),
            ),
        );

        await user.click(within(rowFor("scratch")).getByRole("button", { name: /More actions/ }));
        await user.click(await screen.findByRole("menuitem", { name: "Remove" }));
        const plain = await screen.findByRole("alertdialog");
        expect(within(plain).queryByRole("textbox")).not.toBeInTheDocument();
        expect(within(plain).getByRole("button", { name: /REMOVE/ })).toBeEnabled();
    });

    it("offers menu entries by state", async () => {
        mockCommands();
        renderPage();
        await screen.findByText("scratch");
        const user = userEvent.setup();
        await user.click(within(rowFor("shop_web.1.x7kq")).getByRole("button", { name: /More actions/ }));
        expect(await screen.findByRole("menuitem", { name: "Stop" })).toBeInTheDocument();
        expect(screen.getByRole("menuitem", { name: "Pause" })).not.toHaveAttribute("aria-disabled", "true");
        await user.keyboard("{Escape}");
        await user.click(within(rowFor("scratch")).getByRole("button", { name: /More actions/ }));
        expect(await screen.findByRole("menuitem", { name: "Start" })).toBeInTheDocument();
        expect(screen.getByRole("menuitem", { name: "Pause" })).toHaveAttribute("aria-disabled", "true");
        expect(screen.getByRole("menuitem", { name: "Logs (no service)" })).toHaveAttribute("aria-disabled", "true");
    });

    it("swaps in the bulk bar, confirms STOP with the typed count, and sends one operation", async () => {
        mockCommands({ containers_bulk_action: () => ({ operationId: "018f2e34-0000-7000-8000-0000000000cc", replayed: false }) });
        renderPage();
        await screen.findByText("scratch");
        const user = userEvent.setup();
        await user.click(screen.getByRole("checkbox", { name: "Select all rows" }));
        const bar = screen.getByRole("toolbar", { name: "Bulk actions" });
        expect(within(bar).getByText("2 selected")).toBeInTheDocument();
        expect(within(bar).getByText(/across 2 nodes/)).toBeInTheDocument();
        await user.click(within(bar).getByRole("button", { name: /STOP/ }));
        const dialog = await screen.findByRole("alertdialog");
        const confirm = within(dialog).getByRole("button", { name: /STOP/ });
        expect(confirm).toBeDisabled();
        await user.type(within(dialog).getByRole("textbox"), "2");
        await user.click(confirm);
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith(
                "containers_bulk_action",
                expect.objectContaining({
                    action: "stop",
                    force: false,
                    targets: [
                        { nodeId: "manager", containerId: "a".repeat(64) },
                        { nodeId: "worker-1", containerId: "b".repeat(64) },
                    ],
                    idempotencyKey: expect.stringMatching(/^[0-9a-f-]{36}$/),
                }),
            ),
        );
        await waitFor(() => expect(screen.queryByRole("toolbar", { name: "Bulk actions" })).not.toBeInTheDocument());
    });
});
