import { afterEach, describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRouter, RouterProvider } from "@tanstack/react-router";
import { routeTree } from "@/routeTree.gen";
import { AuthContext, type AuthContextValue } from "@/contexts/auth-context-value";
import volumes from "../fixtures/volumes-ipc.json";
import nodes from "../fixtures/nodes-ipc.json";
import operations from "../fixtures/operations-ipc.json";
import { middleTruncate } from "@/components/volumes/volume-search";
import type { VolumeListView } from "@/types/volumes";
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
const created = {
    nodeId: "worker-1",
    name: "pg-data",
    driver: "local",
    mountPoint: "/var/lib/docker/volumes/pg-data/_data",
    createdAt: "2026-09-28T12:06:00.000Z",
    labels: { app: "billing" },
    usedBy: 0,
    users: [],
    sizeBytes: 0,
};

function mockCommands(overrides: Record<string, (args: Record<string, unknown>) => unknown> = {}) {
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
        if (command in overrides) return overrides[command]((args ?? {}) as Record<string, unknown>);
        switch (command) {
            case "list_volumes":
                return volumes.volumeList;
            case "list_nodes":
                return twoOnline;
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
function renderPage(path = "/volumes") {
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

/** `pg-data` on manager: the name check finds nothing, the create is admitted, and `get_operation` answers `operation()`. */
function mockCreate(operation: () => Record<string, unknown>) {
    mockCommands({
        list_volumes: (args) => (args.name ? { ...volumes.volumeList, items: [] } : volumes.volumeList),
        create_volume: () => ({ operationId: OPERATION, replayed: false }),
        get_operation: () => ({
            ...operations.operation,
            id: OPERATION,
            kind: "volume.create",
            progress: { nodeId: "manager", name: "pg-data" },
            ...operation(),
        }),
    });
}

async function submitCreate(user: ReturnType<typeof userEvent.setup>) {
    await user.click(screen.getByRole("button", { name: /CREATE VOLUME/ }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByLabelText("Name"), "pg-data");
    await user.click(within(dialog).getByRole("button", { name: /CREATE VOLUME/ }));
    await waitFor(() =>
        expect(invoke).toHaveBeenCalledWith("create_volume", expect.objectContaining({ volume: { nodeId: "manager", name: "pg-data", labels: {} } })),
    );
    return dialog;
}

describe("Volumes page", () => {
    beforeEach(() => vi.clearAllMocks());
    afterEach(() => vi.useRealTimers());

    it("lists volumes per node with sizes, mount points, and the storage strip", async () => {
        mockCommands();
        renderPage();
        const redis = rowFor(await screen.findByText("toolbox_redis-data").then((node) => node.textContent ?? ""));
        expect(within(redis).getByText("manager")).toBeInTheDocument();
        expect(within(redis).getByText("512 MB")).toBeInTheDocument();
        expect(within(redis).getByText(middleTruncate("/var/lib/docker/volumes/toolbox_redis-data/_data"))).toBeInTheDocument();
        expect(within(redis).getByRole("button", { name: "Copy mount point of toolbox_redis-data" })).toBeInTheDocument();
        expect(within(rowFor("scratch")).getByText("—")).toBeInTheDocument();
        expect(screen.getByText("2 volumes · 512 MB allocated · sizes pending")).toBeInTheDocument();
        expect(screen.getByText("1 ATTACHED")).toBeInTheDocument();
        expect(screen.getByText("1 UNATTACHED")).toBeInTheDocument();
        expect(screen.getByTestId("growth-trend-skeleton")).toBeInTheDocument();
        expect(within(redis).getByRole("button", { name: "Browse files" })).toHaveAttribute("aria-disabled", "true");
        expect(invoke).toHaveBeenCalledWith("list_volumes", { profileId: session.profileId, nodeId: null, name: null });
    });

    it("disables removal of an attached volume and types the name to remove an unattached one", async () => {
        mockCommands({ remove_volume: () => ({ operationId: OPERATION, replayed: false }) });
        renderPage();
        await screen.findByText("scratch");
        const user = userEvent.setup();
        expect(within(rowFor("toolbox_redis-data")).getByRole("button", { name: "Remove" })).toHaveAttribute("aria-disabled", "true");
        await user.click(within(rowFor("toolbox_redis-data")).getByRole("button", { name: "Remove" }));
        expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();

        await user.click(within(rowFor("scratch")).getByRole("button", { name: "Remove" }));
        const dialog = await screen.findByRole("alertdialog");
        const confirm = within(dialog).getByRole("button", { name: /REMOVE/ });
        expect(confirm).toBeDisabled();
        await user.type(within(dialog).getByRole("textbox"), "scratch");
        await user.click(confirm);
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith("remove_volume", {
                profileId: session.profileId,
                nodeId: "worker-1",
                name: "scratch",
                idempotencyKey: expect.stringMatching(/^[0-9a-f-]{36}$/),
            }),
        );
    });

    it("filters attached volumes from the URL", async () => {
        mockCommands();
        renderPage("/volumes?attached=no");
        expect(await screen.findByText("scratch")).toBeInTheDocument();
        expect(screen.queryByText("toolbox_redis-data")).not.toBeInTheDocument();
        expect(screen.getByText("unattached")).toBeInTheDocument();
    });

    it("validates the name on blur against the chosen node and keeps CREATE disabled while invalid", async () => {
        mockCommands({
            list_volumes: (args) =>
                args.name
                    ? { ...volumes.volumeList, items: args.name === "scratch" && args.nodeId === "worker-1" ? [volumes.volumeList.items[1]] : [] }
                    : volumes.volumeList,
        });
        renderPage();
        await screen.findByText("scratch");
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: /CREATE VOLUME/ }));
        const dialog = await screen.findByRole("dialog");
        const create = within(dialog).getByRole("button", { name: /CREATE VOLUME/ });
        expect(create).toBeDisabled();
        const field = within(dialog).getByLabelText("Name");
        await user.type(field, "a");
        await user.tab();
        expect(await within(dialog).findByRole("alert")).toHaveTextContent("2–255 letters");
        expect(create).toBeDisabled();

        await user.clear(field);
        await user.type(field, "scratch");
        await user.tab();
        // manager is the default node: free there.
        await waitFor(() => expect(invoke).toHaveBeenCalledWith("list_volumes", { profileId: session.profileId, nodeId: "manager", name: "scratch" }));
        expect(within(dialog).queryByRole("alert")).not.toBeInTheDocument();
        expect(create).toBeEnabled();

        await user.click(within(dialog).getByRole("combobox"));
        await user.click(await screen.findByRole("option", { name: "worker-1" }));
        expect(await within(dialog).findByRole("alert")).toHaveTextContent(
            "A volume named scratch already exists on worker-1. Pick another name or another node.",
        );
        expect(create).toBeDisabled();
        expect(within(dialog).getByText("local")).toBeInTheDocument();
    });

    it("creates with labels, waits for the operation, then closes and highlights the new row", async () => {
        let listed = ipcFixture<VolumeListView>(volumes.volumeList);
        mockCommands({
            list_volumes: (args) => (args.name ? { ...volumes.volumeList, items: [] } : listed),
            create_volume: () => {
                listed = { ...listed, items: [...listed.items, created] };
                return { operationId: OPERATION, replayed: false };
            },
            get_operation: () => ({
                ...operations.operation,
                id: OPERATION,
                kind: "volume.create",
                state: "succeeded",
                progress: { nodeId: "worker-1", name: "pg-data" },
            }),
        });
        renderPage();
        await screen.findByText("scratch");
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: /CREATE VOLUME/ }));
        const dialog = await screen.findByRole("dialog");
        await user.type(within(dialog).getByLabelText("Name"), "pg-data");
        await user.click(within(dialog).getByRole("combobox"));
        await user.click(await screen.findByRole("option", { name: "worker-1" }));
        await user.click(within(dialog).getByRole("button", { name: /Add label/ }));
        await user.type(within(dialog).getByRole("textbox", { name: "New label" }), "app=billing{Enter}");
        expect(within(dialog).getByText("app=billing")).toBeInTheDocument();
        await user.click(within(dialog).getByRole("button", { name: /CREATE VOLUME/ }));
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith("create_volume", {
                profileId: session.profileId,
                volume: { nodeId: "worker-1", name: "pg-data", labels: { app: "billing" } },
                idempotencyKey: expect.stringMatching(/^[0-9a-f-]{36}$/),
            }),
        );
        await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
        await screen.findByText("pg-data");
        await waitFor(() => expect(rowFor("pg-data").classList.contains("bg-primary-faint")).toBe(true));
        expect(rowFor("scratch").classList.contains("bg-primary-faint")).toBe(false);
    });

    it("creates on the first click while the name check is still in flight, and says pinning is up to the user", async () => {
        mockCommands({
            list_volumes: (args) => (args.name ? new Promise(() => {}) : volumes.volumeList),
            create_volume: () => ({ operationId: OPERATION, replayed: false }),
        });
        renderPage();
        await screen.findByText("scratch");
        const dialog = await submitCreate(userEvent.setup());
        expect(within(dialog).getByText(/Pin services that mount it to that node with a placement constraint\./)).toBeInTheDocument();
        expect(within(dialog).queryByText(/will be pinned/)).not.toBeInTheDocument();
    });

    it("keeps CREATE disabled while a label fails to parse", async () => {
        mockCommands({ list_volumes: (args) => (args.name ? { ...volumes.volumeList, items: [] } : volumes.volumeList) });
        renderPage();
        await screen.findByText("scratch");
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: /CREATE VOLUME/ }));
        const dialog = await screen.findByRole("dialog");
        await user.type(within(dialog).getByLabelText("Name"), "pg-data");
        await user.click(within(dialog).getByRole("button", { name: /Add label/ }));
        await user.type(within(dialog).getByRole("textbox", { name: "New label" }), "=billing");
        await user.tab();
        expect(await within(dialog).findByText("Labels are key=value, with a key.")).toBeInTheDocument();
        expect(within(dialog).getByRole("button", { name: /CREATE VOLUME/ })).toBeDisabled();
    });

    it("frees CREATE after a failed create once the name is edited", async () => {
        mockCreate(() => ({ state: "failed", errorCode: "conflict", errorMessage: "A volume named pg-data already exists on manager." }));
        renderPage();
        await screen.findByText("scratch");
        const user = userEvent.setup();
        const dialog = await submitCreate(user);
        expect(await within(dialog).findByText("A volume named pg-data already exists on manager.")).toBeInTheDocument();
        const create = within(dialog).getByRole("button", { name: /CREATE VOLUME/ });
        await user.type(within(dialog).getByLabelText("Name"), "-2");
        expect(create).toBeEnabled();
        expect(create).not.toHaveAttribute("aria-busy");
    });

    it("doesn't pull the user back when a create settles after they left the page", async () => {
        let settled = false;
        mockCreate(() => ({ state: settled ? "succeeded" : "running" }));
        const router = renderPage();
        await screen.findByText("scratch");
        await submitCreate(userEvent.setup());
        await waitFor(() => expect(invoke).toHaveBeenCalledWith("get_operation", expect.objectContaining({ operationId: OPERATION })));
        await act(() => router.navigate({ to: "/stacks" }));
        settled = true;
        await waitFor(() => expect(toast.success).toHaveBeenCalledWith("Created pg-data", expect.anything()), { timeout: 3000 });
        await act(() => new Promise((resolve) => setTimeout(resolve, 50)));
        expect(router.state.location.pathname).toBe("/stacks");
    });

    it("doesn't pull the user back when the highlight ends after they left the page", async () => {
        vi.useFakeTimers({ shouldAdvanceTime: true });
        mockCreate(() => ({ state: "succeeded" }));
        const router = renderPage();
        await screen.findByText("scratch");
        await submitCreate(userEvent.setup({ advanceTimers: vi.advanceTimersByTime }));
        await waitFor(() => expect(router.state.location.search).toEqual({ highlightedRow: "manager/pg-data" }));
        await act(() => router.navigate({ to: "/stacks" }));
        await act(() => vi.advanceTimersByTimeAsync(3_500));
        expect(router.state.location.pathname).toBe("/stacks");
    });
});
