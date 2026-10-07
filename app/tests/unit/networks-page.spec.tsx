import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRouter, RouterProvider } from "@tanstack/react-router";
import { routeTree } from "@/routeTree.gen";
import { AuthContext, type AuthContextValue } from "@/contexts/auth-context-value";
import networks from "../fixtures/networks-ipc.json";
import containers from "../fixtures/containers-ipc.json";
import nodes from "../fixtures/nodes-ipc.json";
import operations from "../fixtures/operations-ipc.json";
import type { NetworkListView } from "@/types/networks";
import { ipcFixture } from "../ipc-fixture";

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

const OPERATION = "018f2e34-0000-7000-8000-0000000000c1";
const [managerNode] = nodes.nodeList.items;
const twoOnline = {
    ...nodes.nodeList,
    items: ["manager", "worker-1"].map((id) => ({ ...managerNode, id, hostname: id, agentStatus: "online" })),
};

function mockCommands(overrides: Record<string, (args: Record<string, unknown>) => unknown> = {}) {
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
        if (command in overrides) return overrides[command]((args ?? {}) as Record<string, unknown>);
        switch (command) {
            case "list_networks":
                return networks.networkList;
            case "inspect_network":
                return networks.attached;
            case "list_containers":
                return { ...containers.containerList, nodes: containers.containerList.nodes.map((node) => ({ ...node, status: "ok" })) };
            case "list_nodes":
                return twoOnline;
            case "get_cluster_samples":
                return nodes.clusterSamples;
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

/** Renders the app at `path` and returns its router, so a test can leave the page. */
function renderPage(path = "/networks") {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const router = createRouter({ routeTree, context: { auth }, history: createMemoryHistory({ initialEntries: [path] }) });
    render(
        <AuthContext value={auth}>
            <QueryClientProvider client={queryClient}>
                <RouterProvider router={router} context={{ auth }} />
            </QueryClientProvider>
        </AuthContext>,
    );
    return router;
}

const rowFor = (name: string) => screen.getByText(name).closest("tr") as HTMLElement;

describe("Networks page", () => {
    beforeEach(() => vi.clearAllMocks());

    it("lists networks with scope, subnet, counts, and the overview strip", async () => {
        mockCommands();
        renderPage();
        await screen.findByText("toolbox_kontainer");
        const overlay = rowFor("toolbox_kontainer");
        expect(within(overlay).getByText("x7kq2m9p0r1s")).toBeInTheDocument();
        expect(within(overlay).getByText("swarm")).toBeInTheDocument();
        expect(within(overlay).getByText("10.0.1.0/24")).toBeInTheDocument();
        expect(within(overlay).getByText("6")).toBeInTheDocument();
        expect(within(rowFor("empty-overlay")).getAllByText("—")).toHaveLength(2);
        expect(screen.getByText("3 networks · 2 overlay · 1 bridge")).toBeInTheDocument();
        expect(screen.getByText("1 ACTIVE")).toBeInTheDocument();
        expect(screen.getByText("2 EMPTY")).toBeInTheDocument();
        expect(screen.getByTestId("traffic-trend-skeleton")).toBeInTheDocument();
    });

    it("keeps built-in networks' trash disabled", async () => {
        mockCommands();
        renderPage();
        await screen.findByText("bridge", { selector: "span.font-semibold" });
        const bridge = screen.getByText("bridge", { selector: "span.font-semibold" }).closest("tr") as HTMLElement;
        expect(within(bridge).getByRole("button", { name: "Remove" })).toHaveAttribute("aria-disabled", "true");
    });

    it("explains a blocked removal with the attached names and links to the containers", async () => {
        const onNetwork = {
            ...containers.containerList,
            nodes: containers.containerList.nodes.map((node) => ({ ...node, status: "ok" })),
            items: containers.containerList.items.map((item) => ({ ...item, networks: item.name === "scratch" ? [] : ["toolbox_kontainer"] })),
        };
        mockCommands({ list_containers: () => onNetwork });
        renderPage();
        await screen.findByText("toolbox_kontainer");
        const user = userEvent.setup();
        await user.click(within(rowFor("toolbox_kontainer")).getByRole("button", { name: "Remove" }));
        expect(await screen.findByText("Can't remove toolbox_kontainer")).toBeInTheDocument();
        expect(await screen.findByText("toolbox_nginx.1.x7kq")).toBeInTheDocument();
        expect(screen.getByText("toolbox_redis.1.a2b3")).toBeInTheDocument();
        expect(screen.getByText(/2 containers are still attached/)).toBeInTheDocument();
        expect(invoke).toHaveBeenCalledWith("inspect_network", { profileId: session.profileId, networkId: "x7kq2m9p0r1s3t5u7w9y0z2b4", nodeId: null });
        expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
        await user.click(screen.getByRole("button", { name: /VIEW CONTAINERS/ }));
        expect(await screen.findByText("network: toolbox_kontainer")).toBeInTheDocument();
        expect(screen.getByText("shop_web.1.x7kq")).toBeInTheDocument();
        expect(screen.queryByText("scratch")).not.toBeInTheDocument();
    });

    it("confirms removing an empty network plainly and sends it to the manager", async () => {
        mockCommands({ remove_network: () => ({ operationId: OPERATION, replayed: false }) });
        renderPage();
        await screen.findByText("empty-overlay");
        const user = userEvent.setup();
        await user.click(within(rowFor("empty-overlay")).getByRole("button", { name: "Remove" }));
        const dialog = await screen.findByRole("alertdialog");
        expect(within(dialog).queryByRole("textbox")).not.toBeInTheDocument();
        await user.click(within(dialog).getByRole("button", { name: /REMOVE/ }));
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith("remove_network", {
                profileId: session.profileId,
                networkId: "e1e1e1e1e1e1e1e1e1e1e1e1e",
                nodeId: null,
                idempotencyKey: expect.stringMatching(/^[0-9a-f-]{36}$/),
            }),
        );
    });

    it("validates the dialog and shows node only for bridge and attachable only for overlay", async () => {
        mockCommands();
        renderPage();
        await screen.findByText("toolbox_kontainer");
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: /CREATE NETWORK/ }));
        const dialog = await screen.findByRole("dialog");
        const create = within(dialog).getByRole("button", { name: /CREATE NETWORK/ });
        expect(create).toBeDisabled();
        const name = within(dialog).getByLabelText("Name");
        await user.type(name, "toolbox_kontainer");
        await user.tab();
        expect(await within(dialog).findByRole("alert")).toHaveTextContent("A network named toolbox_kontainer already exists");
        expect(create).toBeDisabled();
        await user.clear(name);
        await user.type(name, "app-network");
        expect(create).toBeEnabled();
        expect(within(dialog).getByRole("switch")).toBeInTheDocument();
        expect(within(dialog).queryByRole("combobox")).not.toBeInTheDocument();
        await user.click(within(dialog).getByRole("radio", { name: "bridge" }));
        expect(within(dialog).queryByRole("switch")).not.toBeInTheDocument();
        expect(within(dialog).getByRole("combobox")).toBeInTheDocument();
        const subnet = within(dialog).getByLabelText("Subnet · optional");
        await user.type(subnet, "10.0.9.1/24");
        await user.tab();
        expect(await within(dialog).findByText("A network address with its prefix, like 10.0.9.0/24.")).toBeInTheDocument();
        expect(create).toBeDisabled();
    });

    it("creates an overlay, waits for the operation, then closes and highlights the row", async () => {
        let listed = ipcFixture<NetworkListView>(networks.networkList);
        const createdRow = { ...listed.items[2]!, id: "n1n1n1n1n1n1n1n1n1n1n1n1n", name: "app-network" };
        mockCommands({
            list_networks: () => listed,
            create_network: () => {
                listed = { ...listed, items: [...listed.items, createdRow] };
                return { operationId: OPERATION, replayed: false };
            },
            get_operation: () => ({
                ...operations.operation,
                id: OPERATION,
                kind: "network.create",
                state: "succeeded",
                progress: { networkId: createdRow.id, name: "app-network", nodeId: null },
            }),
        });
        renderPage();
        await screen.findByText("toolbox_kontainer");
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: /CREATE NETWORK/ }));
        const dialog = await screen.findByRole("dialog");
        await user.type(within(dialog).getByLabelText("Name"), "app-network");
        await user.type(within(dialog).getByLabelText("Subnet · optional"), "10.0.9.0/24");
        await user.click(within(dialog).getByRole("button", { name: /CREATE NETWORK/ }));
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith("create_network", {
                profileId: session.profileId,
                network: { name: "app-network", driver: "overlay", attachable: true, nodeId: null, subnet: "10.0.9.0/24" },
                idempotencyKey: expect.stringMatching(/^[0-9a-f-]{36}$/),
            }),
        );
        await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
        await screen.findByText("app-network");
        await waitFor(() => expect(rowFor("app-network").classList.contains("bg-primary-faint")).toBe(true));
    });

    it("doesn't pull the user back when a create settles after they left the page", async () => {
        let settled = false;
        mockCommands({
            create_network: () => ({ operationId: OPERATION, replayed: false }),
            get_operation: () => ({
                ...operations.operation,
                id: OPERATION,
                kind: "network.create",
                state: settled ? "succeeded" : "running",
                progress: { networkId: "n1n1n1n1n1n1n1n1n1n1n1n1n", name: "app-network", nodeId: null },
            }),
        });
        const router = renderPage();
        await screen.findByText("toolbox_kontainer");
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: /CREATE NETWORK/ }));
        const dialog = await screen.findByRole("dialog");
        await user.type(within(dialog).getByLabelText("Name"), "app-network");
        await user.click(within(dialog).getByRole("button", { name: /CREATE NETWORK/ }));
        await waitFor(() => expect(invoke).toHaveBeenCalledWith("get_operation", expect.objectContaining({ operationId: OPERATION })));
        await act(() => router.navigate({ to: "/stacks" }));
        settled = true;
        await waitFor(() => expect(toast.success).toHaveBeenCalled(), { timeout: 3000 });
        await act(() => new Promise((resolve) => setTimeout(resolve, 50)));
        expect(router.state.location.pathname).toBe("/stacks");
    });
});
