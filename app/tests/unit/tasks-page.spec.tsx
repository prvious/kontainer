import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import { Route as TasksRoute } from "@/routes/_authenticated/tasks/index";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import fixture from "../fixtures/inventory-ipc.json";
import nodesFixture from "../fixtures/nodes-ipc.json";

/** The node list with `manager` renamed `node-1`, the id the inventory fixture uses. */
const nodeList = {
    ...nodesFixture.nodeList,
    items: nodesFixture.nodeList.items.map((node) => (node.id === "manager" ? { ...node, id: "node-1" } : node)),
};

const profileId = "018f2e34-0000-7000-8000-0000000000bb";
const reportInventoryCustodyError = vi.fn();
const auth = { session: { profileId }, reportInventoryCustodyError } as unknown as AuthContextValue;

function renderPage() {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const rootRoute = createRootRoute({});
    const tasksRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/tasks",
        component: TasksRoute.options.component,
    });
    const router = createRouter({
        routeTree: rootRoute.addChildren([tasksRoute]),
        history: createMemoryHistory({ initialEntries: ["/tasks"] }),
    });
    return render(
        <AuthContext value={auth}>
            <QueryClientProvider client={queryClient}>
                <RouterProvider router={router} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

describe("read-only Containers screen", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("shows a loading state, discovers the node, then displays real container fields", async () => {
        let resolveNode!: (value: unknown) => void;
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "list_nodes")
                return new Promise((resolve) => {
                    resolveNode = resolve;
                });
            if (command === "list_container_inventory") return fixture.containerPage;
            throw new Error("unexpected command");
        });
        renderPage();
        expect(await screen.findByLabelText("Loading containers")).toBeInTheDocument();
        resolveNode(nodeList);
        expect(await screen.findByText("web")).toBeInTheDocument();
        expect(screen.getByText("nginx:latest")).toBeInTheDocument();
        expect(screen.getByText("running")).toBeInTheDocument();
        expect(screen.getByText("Up 3 minutes")).toBeInTheDocument();
        expect(screen.getByText("2026-09-19T06:00:00.000Z")).toBeInTheDocument();
        expect(screen.getByText("Node node-1 · read-only inventory")).toBeInTheDocument();
        expect(invoke).toHaveBeenCalledWith("list_container_inventory", {
            profileId,
            nodeId: "node-1",
            cursor: null,
            limit: 50,
        });
        expect(screen.queryByRole("button", { name: /logs|terminal|exec|remove/i })).not.toBeInTheDocument();
    });

    it("shows a no-node state without calling inventory when nothing is online", async () => {
        const offline = { ...nodeList, items: nodeList.items.map((node) => ({ ...node, status: "offline" })) };
        vi.mocked(invoke).mockImplementation(async (command: string) => (command === "list_nodes" ? offline : null));
        renderPage();
        expect(await screen.findByText("No node is online. Start an agent and it appears here within seconds.")).toBeInTheDocument();
        expect(
            vi
                .mocked(invoke)
                .mock.calls.map(([command]) => command)
                .filter((command) => command !== "load_app_settings"),
        ).toEqual(["list_nodes"]);
    });

    it("shows an empty page and advances and returns through opaque cursors", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
            if (command === "list_nodes") return nodeList;
            if (command === "list_container_inventory") {
                const cursor = (args as { cursor: string | null }).cursor;
                return cursor ? { items: [], nextCursor: null } : fixture.containerPage;
            }
            throw new Error("unexpected command");
        });
        renderPage();
        await screen.findByText("web");
        await userEvent.setup().click(screen.getByRole("button", { name: "Next" }));
        expect(await screen.findByText("No containers on this page.")).toBeInTheDocument();
        expect(screen.getByText("Page 2")).toBeInTheDocument();
        expect(invoke).toHaveBeenCalledWith("list_container_inventory", {
            profileId,
            nodeId: "node-1",
            cursor: fixture.containerPage.nextCursor,
            limit: 50,
        });
        await userEvent.setup().click(screen.getByRole("button", { name: "Previous" }));
        expect(await screen.findByText("web")).toBeInTheDocument();
    });

    it.each([
        ["agent_unavailable", "The node agent is unavailable."],
        ["agent_timeout", "The node agent did not respond in time."],
        ["controller_unavailable", "The controller is unavailable."],
        ["vault_unavailable", "The credential vault is unavailable."],
        ["relogin_required", "Your session expired or was revoked."],
    ])("shows the %s recovery state without raw error text", async (code, message) => {
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "list_nodes") return nodeList;
            if (command === "list_container_inventory") throw code;
            throw new Error("unexpected command");
        });
        renderPage();
        await waitFor(() => expect(screen.getByText((text) => text.startsWith(message))).toBeInTheDocument());
        expect(screen.queryByText("secret-canary")).not.toBeInTheDocument();
        expect(vi.mocked(invoke).mock.calls.filter(([command]) => command !== "load_app_settings")).toHaveLength(2);
    });

    it("resets an invalid cursor to the first page", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
            if (command === "list_nodes") return nodeList;
            if (command === "list_container_inventory") {
                if ((args as { cursor: string | null }).cursor) throw "invalid_pagination";
                return fixture.containerPage;
            }
            throw new Error("unexpected command");
        });
        renderPage();
        await screen.findByText("web");
        await userEvent.setup().click(screen.getByRole("button", { name: "Next" }));
        await screen.findByText("This page is no longer available. Start again from page one.");
        await userEvent.setup().click(screen.getByRole("button", { name: "First page" }));
        expect(await screen.findByText("web")).toBeInTheDocument();
    });
});
