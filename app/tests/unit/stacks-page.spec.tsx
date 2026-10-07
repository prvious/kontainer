import { useState } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRouter, RouterProvider } from "@tanstack/react-router";
import { routeTree } from "@/routeTree.gen";
import { AuthContext, type AuthContextValue } from "@/contexts/auth-context-value";
import stacks from "../fixtures/stacks-ipc.json";
import services from "../fixtures/services-ipc.json";
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

const OPERATION = "018f2e34-0000-7000-8000-0000000000c1";

function mockCommands(overrides: Record<string, (args: Record<string, unknown>) => unknown> = {}) {
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
        if (command in overrides) return overrides[command]((args ?? {}) as Record<string, unknown>);
        switch (command) {
            case "list_stacks":
                return stacks.stackList;
            case "get_stack_removal_plan":
                return stacks.plan;
            case "list_services":
                return services.serviceList;
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

function renderPage(path = "/stacks") {
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

describe("Stacks page", () => {
    beforeEach(() => vi.clearAllMocks());

    it("lists stacks with status, counts, and figures", async () => {
        mockCommands();
        renderPage();
        await screen.findByText("toolbox");
        expect(screen.getByText("3 stacks deployed · 6 services · 8 containers")).toBeInTheDocument();
        const toolbox = rowFor("toolbox");
        expect(within(toolbox).getByText("partial")).toBeInTheDocument();
        expect(within(toolbox).getByText("4/5")).toBeInTheDocument();
        expect(within(toolbox).getByText("0.5%")).toBeInTheDocument();
        expect(within(toolbox).getByText("20 MB")).toBeInTheDocument();
        expect(within(rowFor("idle")).getByText("stopped")).toBeInTheDocument();
        expect(within(rowFor("idle")).getAllByText("—").length).toBeGreaterThan(0);
        expect(screen.getByRole("button", { name: /DEPLOY STACK/ })).toHaveAttribute("data-restricted", "true");
    });

    it("explains the empty cluster with the docs link and hides the toolbar", async () => {
        mockCommands({ list_stacks: () => ({ ...stacks.stackList, items: [] }) });
        vi.mocked(openUrl).mockResolvedValue(undefined);
        renderPage();
        expect(await screen.findByText("No stacks deployed")).toBeInTheDocument();
        expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
        await userEvent.setup().click(screen.getByRole("button", { name: /DOCKER DOCS/ }));
        expect(openUrl).toHaveBeenCalledWith("https://docs.docker.com/engine/swarm/stack-deploy/");
    });

    it("shows a stack's services from the row menu", async () => {
        mockCommands();
        renderPage();
        await screen.findByText("toolbox");
        const user = userEvent.setup();
        await user.click(within(rowFor("toolbox")).getByRole("button", { name: /More actions/ }));
        await user.click(await screen.findByRole("menuitem", { name: "Show services" }));
        expect(await screen.findByText("stack: toolbox")).toBeInTheDocument();
        expect(screen.getByText("toolbox_alpine")).toBeInTheDocument();
        expect(screen.queryByText("webapp_web")).not.toBeInTheDocument();
    });

    it("loads the plan, needs the typed name, shows the phase, and closes on success", async () => {
        let polls = 0;
        mockCommands({
            remove_stack: () => ({ operationId: OPERATION, replayed: false }),
            get_operation: () => {
                polls += 1;
                return polls < 2
                    ? {
                          ...operations.operation,
                          id: OPERATION,
                          kind: "stack.remove",
                          state: "running",
                          progress: { phase: "waiting_for_tasks", servicesRemoved: 3, networksRemoved: 0, volumesRetained: ["toolbox_redis-data"] },
                      }
                    : {
                          ...operations.operation,
                          id: OPERATION,
                          kind: "stack.remove",
                          state: "succeeded",
                          finishedAt: "2026-09-28T12:00:05.000Z",
                          progress: { phase: "removed", servicesRemoved: 3, networksRemoved: 1, volumesRetained: ["toolbox_redis-data"] },
                      };
            },
        });
        renderPage();
        await screen.findByText("toolbox");
        const user = userEvent.setup();
        await user.click(within(rowFor("toolbox")).getByRole("button", { name: /More actions/ }));
        await user.click(await screen.findByRole("menuitem", { name: "Remove" }));
        const dialog = await screen.findByRole("alertdialog");
        expect(await within(dialog).findByText("toolbox_alpine · toolbox_nginx · toolbox_redis")).toBeInTheDocument();
        expect(within(dialog).getByText("toolbox_kontainer")).toBeInTheDocument();
        expect(within(dialog).getByText("toolbox_redis-data")).toBeInTheDocument();
        expect(invoke).toHaveBeenCalledWith("get_stack_removal_plan", { profileId: session.profileId, name: "toolbox" });
        const confirm = within(dialog).getByRole("button", { name: /REMOVE STACK/ });
        expect(confirm).toBeDisabled();
        await user.type(within(dialog).getByRole("textbox"), "toolbox");
        await user.click(confirm);
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith("remove_stack", {
                profileId: session.profileId,
                name: "toolbox",
                idempotencyKey: expect.stringMatching(/^[0-9a-f-]{36}$/),
            }),
        );
        expect(await within(dialog).findByText("Waiting for tasks to stop")).toBeInTheDocument();
        await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument(), { timeout: 3000 });
    });

    /** Opens the remove dialog for toolbox, types the name, and confirms; `settle` is the operation's final snapshot. */
    async function removeToolbox(settle: Record<string, unknown>) {
        let plans = 0;
        let polls = 0;
        mockCommands({
            // Once the services are gone the stack no longer exists, so the refreshed plan fails.
            get_stack_removal_plan: () => {
                plans += 1;
                if (plans > 1) throw "not_found";
                return stacks.plan;
            },
            remove_stack: () => ({ operationId: OPERATION, replayed: false }),
            get_operation: () => {
                polls += 1;
                const base = { ...operations.operation, id: OPERATION, kind: "stack.remove" };
                return polls < 2
                    ? { ...base, state: "running", progress: { phase: "removing_networks", servicesRemoved: 3, networksRemoved: 0, volumesRetained: [] } }
                    : {
                          ...base,
                          finishedAt: "2026-09-28T12:01:05.000Z",
                          progress: { phase: "removing_networks", servicesRemoved: 3, networksRemoved: 0 },
                          ...settle,
                      };
            },
        });
        renderPage();
        await screen.findByText("toolbox");
        const user = userEvent.setup();
        await user.click(within(rowFor("toolbox")).getByRole("button", { name: /More actions/ }));
        await user.click(await screen.findByRole("menuitem", { name: "Remove" }));
        const dialog = await screen.findByRole("alertdialog");
        await within(dialog).findByText("toolbox_alpine · toolbox_nginx · toolbox_redis");
        await user.type(within(dialog).getByRole("textbox"), "toolbox");
        await user.click(within(dialog).getByRole("button", { name: /REMOVE STACK/ }));
        return dialog;
    }

    it("shows a failure after the services went with the real message and never says nothing changed", async () => {
        const message = "Removed 3 services, but network toolbox_kontainer is still in use. Remove it from Networks once it's empty.";
        const dialog = await removeToolbox({ state: "failed", errorCode: "network_in_use", errorMessage: message });
        expect(await within(dialog).findByRole("alert", {}, { timeout: 3000 })).toHaveTextContent(message);
        expect(await within(dialog).findByText(/The removal plan couldn't be loaded \(not_found\)/)).toBeInTheDocument();
        expect(within(dialog).queryByText(/Nothing was changed/)).not.toBeInTheDocument();
        expect(within(dialog).getByRole("link", { name: "OPEN NETWORKS" })).toHaveAttribute("href", "/networks");
    });

    it("says an uncertain removal is uncertain", async () => {
        const dialog = await removeToolbox({ state: "uncertain" });
        expect(await within(dialog).findByRole("alert", {}, { timeout: 3000 })).toHaveTextContent("Outcome uncertain — check the stack before retrying.");
        expect(within(dialog).queryByText(/Nothing was changed/)).not.toBeInTheDocument();
        expect(within(dialog).queryByRole("link", { name: "OPEN NETWORKS" })).not.toBeInTheDocument();
    });
});
