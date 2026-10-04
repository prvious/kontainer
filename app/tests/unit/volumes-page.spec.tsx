import { useState } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
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

function Harness({ path }: { path: string }) {
    const [router] = useState(() => createRouter({ routeTree, context: { auth }, history: createMemoryHistory({ initialEntries: [path] }) }));
    return <RouterProvider router={router} context={{ auth }} />;
}

function renderPage(path = "/volumes") {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    return render(
        <AuthContext value={auth}>
            <QueryClientProvider client={queryClient}>
                <Harness path={path} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

const rowFor = (name: string) => screen.getByText(name).closest("tr") as HTMLElement;

describe("Volumes page", () => {
    beforeEach(() => vi.clearAllMocks());

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
});
