import { useState } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRouter, RouterProvider } from "@tanstack/react-router";
import { routeTree } from "@/routeTree.gen";
import { AuthContext, type AuthContextValue } from "@/contexts/auth-context-value";
import images from "../fixtures/images-ipc.json";
import containers from "../fixtures/containers-ipc.json";
import nodes from "../fixtures/nodes-ipc.json";
import operations from "../fixtures/operations-ipc.json";

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

const FIRST = "018f2e34-0000-7000-8000-0000000000c1";
const RETRY = "018f2e34-0000-7000-8000-0000000000c2";
const [managerNode] = nodes.nodeList.items;
const threeOnline = {
    ...nodes.nodeList,
    items: ["manager", "worker-1", "worker-2"].map((id) => ({ ...managerNode, id, hostname: id, agentStatus: "online" })),
};
const containerList = {
    ...containers.containerList,
    nodes: containers.containerList.nodes.map((node) => ({ ...node, status: "ok" })),
};

const pull = (nodeId: string, status: string, extra: Record<string, unknown> = {}) => ({
    nodeId,
    status,
    layersDone: status === "done" ? 3 : 1,
    layersTotal: 3,
    bytes: status === "done" ? 3_600_000 : 1_200_000,
    totalBytes: 3_600_000,
    ...extra,
});
const operation = (id: string, state: string, progress: unknown, errorCode: string | null = null, errorMessage: string | null = null) => ({
    ...operations.operation,
    id,
    kind: "image.pull",
    state,
    progress,
    errorCode,
    errorMessage,
    finishedAt: state === "running" ? null : "2026-09-28T12:00:05.000Z",
});

function mockCommands(overrides: Record<string, (args: Record<string, unknown>) => unknown> = {}) {
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
        if (command in overrides) return overrides[command]((args ?? {}) as Record<string, unknown>);
        switch (command) {
            case "list_images":
                return images.imageList;
            case "list_containers":
                return containerList;
            case "list_nodes":
                return threeOnline;
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

function renderPage(path = "/images") {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    return render(
        <AuthContext value={auth}>
            <QueryClientProvider client={queryClient}>
                <Harness path={path} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

const rowFor = (text: string) => screen.getByText(text).closest("tr") as HTMLElement;

describe("Images page", () => {
    beforeEach(() => vi.clearAllMocks());

    it("lists merged images with their nodes, the storage strip, and the header totals", async () => {
        mockCommands();
        renderPage();
        const nginx = rowFor(await screen.findByText("nginx:1.27").then((node) => node.textContent ?? ""));
        expect(within(nginx).getByText("aaaaaaaaaaaa")).toBeInTheDocument();
        expect(within(nginx).getByText("183 MB")).toBeInTheDocument();
        expect(within(nginx).getByText("manager")).toBeInTheDocument();
        expect(within(nginx).getByText("worker-1")).toBeInTheDocument();
        expect(within(rowFor("<none>")).getByText("0")).toBeInTheDocument();
        expect(screen.getByText("2 images · 191 MB total")).toBeInTheDocument();
        expect(screen.getByText("1 IN USE")).toBeInTheDocument();
        expect(screen.getByText("1 UNUSED")).toBeInTheDocument();
        expect(screen.getByText("DISK USAGE")).toBeInTheDocument();
        expect(screen.getByTestId("pulls-trend-skeleton")).toBeInTheDocument();
        expect(screen.getByText(/worker-2 didn't answer/)).toBeInTheDocument();
        expect(invoke).toHaveBeenCalledWith("list_images", { profileId: session.profileId });
    });

    it("searches by tag or id from the URL", async () => {
        mockCommands();
        renderPage("/images?q=cccc");
        expect(await screen.findByText("<none>")).toBeInTheDocument();
        expect(screen.queryByText("nginx:1.27")).not.toBeInTheDocument();
    });

    it("types the full name:tag to delete an image in use, and names the containers using it", async () => {
        mockCommands({ remove_image: () => ({ operationId: FIRST, replayed: false }) });
        renderPage();
        await screen.findByText("nginx:1.27");
        const user = userEvent.setup();
        await user.click(within(rowFor("nginx:1.27")).getByRole("button", { name: "Delete" }));
        const dialog = await screen.findByRole("alertdialog");
        expect(within(dialog).getByText("Delete image nginx:1.27?")).toBeInTheDocument();
        expect(within(dialog).getByText(/In use by 3 containers: shop_web.1.x7kq/)).toBeInTheDocument();
        const confirm = within(dialog).getByRole("button", { name: /DELETE IMAGE/ });
        expect(confirm).toBeDisabled();
        await user.type(within(dialog).getByRole("textbox"), "nginx:1.2");
        expect(confirm).toBeDisabled();
        await user.type(within(dialog).getByRole("textbox"), "7");
        await user.click(confirm);
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith("remove_image", {
                profileId: session.profileId,
                imageId: images.imageList.items[0]!.imageId,
                nodeIds: "all",
                force: true,
                idempotencyKey: expect.stringMatching(/^[0-9a-f-]{36}$/),
            }),
        );
    });

    it("deletes an unused image with a plain confirmation", async () => {
        mockCommands({ remove_image: () => ({ operationId: FIRST, replayed: false }) });
        renderPage();
        await screen.findByText("<none>");
        const user = userEvent.setup();
        await user.click(within(rowFor("<none>")).getByRole("button", { name: "Delete" }));
        const dialog = await screen.findByRole("alertdialog");
        expect(within(dialog).queryByRole("textbox")).not.toBeInTheDocument();
        await user.click(within(dialog).getByRole("button", { name: /^DELETE$/ }));
        await waitFor(() => expect(invoke).toHaveBeenCalledWith("remove_image", expect.objectContaining({ nodeIds: "all", force: false })));
    });

    it("forces the delete of an unused image tagged more than once, which Docker otherwise refuses", async () => {
        const [, unused] = images.imageList.items;
        const twiceTagged = { ...unused!, repoTags: ["redis:7", "redis:latest"], dangling: false };
        mockCommands({
            list_images: () => ({ ...images.imageList, items: [twiceTagged] }),
            remove_image: () => ({ operationId: FIRST, replayed: false }),
        });
        renderPage();
        const user = userEvent.setup();
        await user.click(within(rowFor(await screen.findByText("redis:7").then((node) => node.textContent ?? ""))).getByRole("button", { name: "Delete" }));
        const dialog = await screen.findByRole("alertdialog");
        expect(within(dialog).queryByRole("textbox")).not.toBeInTheDocument();
        await user.click(within(dialog).getByRole("button", { name: /^DELETE$/ }));
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith("remove_image", expect.objectContaining({ imageId: twiceTagged.imageId, nodeIds: "all", force: true })),
        );
    });

    it("validates the reference on blur and pulls on every online node by default", async () => {
        mockCommands({
            pull_image: () => ({ operationId: FIRST, replayed: false }),
            get_operation: () => operation(FIRST, "succeeded", { nodes: ["manager", "worker-1", "worker-2"].map((id) => pull(id, "done")) }),
        });
        renderPage();
        await screen.findByText("nginx:1.27");
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: /PULL IMAGE/ }));
        const dialog = await screen.findByRole("dialog");
        const field = within(dialog).getByLabelText("Image reference");
        await user.type(field, "Alpine");
        await user.tab();
        expect(await within(dialog).findByRole("alert")).toHaveTextContent("Repository names are lowercase");
        expect(invoke).not.toHaveBeenCalledWith("pull_image", expect.anything());
        for (const id of ["manager", "worker-1", "worker-2"]) expect(within(dialog).getByRole("checkbox", { name: `Pull on ${id}` })).toBeChecked();

        await user.clear(field);
        await user.type(field, "alpine:3.20");
        await user.click(within(dialog).getByRole("button", { name: "PULL" }));
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith("pull_image", {
                profileId: session.profileId,
                reference: "alpine:3.20",
                nodeIds: ["manager", "worker-1", "worker-2"],
                idempotencyKey: expect.stringMatching(/^[0-9a-f-]{36}$/),
            }),
        );
        for (const id of ["manager", "worker-1", "worker-2"])
            await waitFor(() => expect(screen.getByTestId(`pull-node-${id}`)).toHaveAttribute("data-state", "done"));
        expect(within(dialog).getByText("3 of 3 nodes done")).toBeInTheDocument();
        await waitFor(() => expect(toast.success).toHaveBeenCalledWith("Pulled alpine:3.20", expect.objectContaining({ id: FIRST })));
    });

    it("shows a failing node's registry error with the hint and retries only that node", async () => {
        mockCommands({
            pull_image: (args) => ({ operationId: (args.nodeIds as string[]).length === 1 ? RETRY : FIRST, replayed: false }),
            get_operation: (args) =>
                args.operationId === FIRST
                    ? operation(
                          FIRST,
                          "failed",
                          {
                              nodes: [
                                  pull("manager", "done"),
                                  pull("worker-1", "done"),
                                  pull("worker-2", "failed", { error: "unauthorized: authentication required" }),
                              ],
                          },
                          "partial_failure",
                          "The pull failed on 1 of 3 nodes.",
                      )
                    : operation(RETRY, "succeeded", { nodes: [pull("worker-2", "done")] }),
        });
        renderPage();
        await screen.findByText("nginx:1.27");
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: /PULL IMAGE/ }));
        const dialog = await screen.findByRole("dialog");
        await user.type(within(dialog).getByLabelText("Image reference"), "ghcr.io/acme/billing:2.4.0");
        await user.click(within(dialog).getByRole("button", { name: "PULL" }));
        const failed = await screen.findByTestId("pull-node-worker-2");
        await waitFor(() => expect(failed).toHaveAttribute("data-state", "failed"));
        expect(within(failed).getByText("unauthorized: authentication required")).toBeInTheDocument();
        expect(within(failed).getByText("Private registries need credentials, not built yet.")).toBeInTheDocument();
        expect(within(dialog).getByText("2 of 3 nodes done · 1 failed")).toBeInTheDocument();

        await user.click(within(failed).getByRole("button", { name: /RETRY/ }));
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith("pull_image", expect.objectContaining({ reference: "ghcr.io/acme/billing:2.4.0", nodeIds: ["worker-2"] })),
        );
        await waitFor(() => expect(screen.getByTestId("pull-node-worker-2")).toHaveAttribute("data-state", "done"));
        expect(within(dialog).getByText("3 of 3 nodes done")).toBeInTheDocument();
    });

    it("hands a running pull to a loading toast when run in the background", async () => {
        mockCommands({
            pull_image: () => ({ operationId: FIRST, replayed: false }),
            get_operation: () => operation(FIRST, "running", { nodes: [pull("manager", "pulling")] }),
        });
        renderPage();
        await screen.findByText("nginx:1.27");
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: /PULL IMAGE/ }));
        const dialog = await screen.findByRole("dialog");
        await user.click(within(dialog).getByRole("checkbox", { name: "Pull on worker-1" }));
        await user.click(within(dialog).getByRole("checkbox", { name: "Pull on worker-2" }));
        await user.type(within(dialog).getByLabelText("Image reference"), "alpine");
        await user.click(within(dialog).getByRole("button", { name: "PULL" }));
        await waitFor(() => expect(screen.getByTestId("pull-node-manager")).toHaveAttribute("data-state", "pulling"));
        expect(within(dialog).getByText("layer 1/3 · 33% · 1.1 MB/3.4 MB")).toBeInTheDocument();
        expect(within(dialog).getByRole("button", { name: /CANCEL PULL/ })).toHaveAttribute("data-restricted", "true");
        await user.click(within(dialog).getByRole("button", { name: /RUN IN BACKGROUND/ }));
        expect(toast.loading).toHaveBeenCalledWith("Pulling alpine", expect.objectContaining({ id: FIRST }));
        await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
        expect(screen.getByRole("button", { name: /PULLING…/ })).toBeInTheDocument();
    });
});
