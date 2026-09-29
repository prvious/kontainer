import { useState } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRouter, RouterProvider } from "@tanstack/react-router";
import { routeTree } from "@/routeTree.gen";
import { AuthContext, type AuthContextValue } from "@/contexts/auth-context-value";
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
const WEB = "webappweb0000000000000000";

function mockCommands(overrides: Record<string, (args: Record<string, unknown>) => unknown> = {}) {
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
        if (command in overrides) return overrides[command]((args ?? {}) as Record<string, unknown>);
        switch (command) {
            case "list_services":
                return services.serviceList;
            case "get_service":
                return services.serviceDetail;
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

function renderPage(path = "/services") {
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

describe("Services page", () => {
    beforeEach(() => vi.clearAllMocks());

    it("lists services with status, mode, replicas, ports, and the summary", async () => {
        mockCommands();
        renderPage();
        await screen.findByText("webapp_web");
        expect(screen.getByText("3 services · 2 replicated · 1 global · 2 services degraded")).toBeInTheDocument();
        const web = rowFor("webapp_web");
        expect(within(web).getByText("partial")).toBeInTheDocument();
        expect(within(web).getByText("nginx:alpine")).toBeInTheDocument();
        expect(within(web).getByText("2/3")).toBeInTheDocument();
        expect(within(web).getByText("8080:80")).toBeInTheDocument();
        expect(within(web).getByText("webapp")).toBeInTheDocument();
        expect(within(rowFor("toolbox_alpine")).getByText("updating")).toBeInTheDocument();
        expect(within(rowFor("monitoring_collector")).getByText("running")).toBeInTheDocument();
        expect(screen.getByRole("button", { name: /CREATE SERVICE/ })).toHaveAttribute("data-restricted", "true");
    });

    it("names the newest task error when a partial count is hovered", async () => {
        mockCommands();
        renderPage();
        await screen.findByText("webapp_web");
        await userEvent.setup().hover(within(rowFor("webapp_web")).getByText("2/3"));
        await waitFor(() => expect(invoke).toHaveBeenCalledWith("get_service", { profileId: session.profileId, serviceId: WEB }));
        expect(await screen.findByText("no suitable node (insufficient memory on 3 nodes)")).toBeInTheDocument();
        expect(screen.getByText("1 of 3 tasks pending")).toBeInTheDocument();
    });

    it("scales inline: the stepper submits on Enter with the row's version, Esc cancels", async () => {
        let polled = 0;
        mockCommands({
            scale_service: () => ({ operationId: OPERATION, replayed: false }),
            get_operation: () => {
                polled += 1;
                return {
                    ...operations.operation,
                    id: OPERATION,
                    kind: "service.scale",
                    state: "running",
                    progress: { desired: 2, running: 1, phase: "converging" },
                };
            },
        });
        renderPage();
        await screen.findByText("monitoring_collector");
        const user = userEvent.setup();
        await user.click(within(rowFor("toolbox_alpine")).getByRole("button", { name: "Scale toolbox_alpine" }));
        const stepper = within(rowFor("toolbox_alpine")).getByRole("group", { name: "Replicas for toolbox_alpine" });
        await user.keyboard("{ArrowUp}{ArrowUp}{Escape}");
        expect(within(rowFor("toolbox_alpine")).queryByRole("group")).not.toBeInTheDocument();
        expect(invoke).not.toHaveBeenCalledWith("scale_service", expect.anything());
        expect(stepper).not.toBeInTheDocument();

        await user.click(within(rowFor("toolbox_alpine")).getByRole("button", { name: "Scale toolbox_alpine" }));
        await user.click(within(rowFor("toolbox_alpine")).getByRole("button", { name: "More replicas" }));
        await user.keyboard("{Enter}");
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith("scale_service", {
                profileId: session.profileId,
                serviceId: "toolboxalpine000000000000",
                replicas: 2,
                version: 12,
                idempotencyKey: expect.stringMatching(/^[0-9a-f-]{36}$/),
            }),
        );
        await waitFor(() => expect(polled).toBeGreaterThan(0));
        const row = rowFor("toolbox_alpine");
        expect(await within(row).findByText("scaling")).toBeInTheDocument();
        expect(within(row).getByText("1/2")).toBeInTheDocument();
        expect(within(row).getByRole("img", { name: "1 of 2 tasks running" })).toBeInTheDocument();
        expect(await screen.findByText("Scaling toolbox_alpine · 1 → 2 replicas")).toBeInTheDocument();
    });

    it("keeps a failed scale pinned with its reason", async () => {
        mockCommands({
            scale_service: () => ({ operationId: OPERATION, replayed: false }),
            get_operation: () => ({
                ...operations.operation,
                id: OPERATION,
                kind: "service.scale",
                state: "failed",
                errorCode: "converge_timeout",
                errorMessage: "webapp_web didn't converge within 60 s: 2 of 3 running.",
                requestId: "01J9ZM2R",
                finishedAt: "2026-09-28T12:01:00.000Z",
                progress: { desired: 3, running: 2, phase: "converging" },
            }),
        });
        renderPage();
        await screen.findByText("webapp_web");
        const user = userEvent.setup();
        await user.click(within(rowFor("webapp_web")).getByRole("button", { name: "Scale webapp_web" }));
        await user.keyboard("{ArrowUp}{Enter}");
        expect(await screen.findByText(/converge_timeout — webapp_web didn't converge within 60 s/)).toBeInTheDocument();
        expect(screen.getByText(/req 01J9ZM2R/)).toBeInTheDocument();
    });

    it("confirms restart and rollback with the row's version", async () => {
        mockCommands({ restart_service: () => ({ operationId: OPERATION, replayed: false }) });
        renderPage();
        await screen.findByText("webapp_web");
        const user = userEvent.setup();
        await user.click(within(rowFor("webapp_web")).getByRole("button", { name: /More actions/ }));
        await user.click(await screen.findByRole("menuitem", { name: "Restart" }));
        const dialog = await screen.findByRole("alertdialog");
        await user.click(within(dialog).getByRole("button", { name: /RESTART/ }));
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith("restart_service", {
                profileId: session.profileId,
                serviceId: WEB,
                version: 7,
                idempotencyKey: expect.stringMatching(/^[0-9a-f-]{36}$/),
            }),
        );
        await user.click(within(rowFor("webapp_web")).getByRole("button", { name: /More actions/ }));
        await user.click(await screen.findByRole("menuitem", { name: "Logs" }));
        // The Logs page opens the service's stream and drops the parameter.
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith("logs_open", expect.objectContaining({ profileId: session.profileId, serviceId: WEB, tail: 500 })),
        );
        expect(await screen.findByRole("heading", { name: "LOGS", level: 1 })).toBeInTheDocument();
    });

    it("shows quorum loss as an error state with a countdown and pauses writes", async () => {
        mockCommands({
            list_services: () => {
                throw "quorum_unavailable";
            },
        });
        renderPage();
        expect(await screen.findByText("Couldn't load services")).toBeInTheDocument();
        expect(screen.getByText(/Your workloads keep running/)).toBeInTheDocument();
        expect(screen.getByText(/quorum_unavailable/)).toBeInTheDocument();
        expect(screen.getByRole("button", { name: /RETRY IN \d+s/ })).toBeInTheDocument();
        const create = screen.getByRole("button", { name: /CREATE SERVICE/ });
        expect(create).toHaveAttribute("data-restricted", "true");
        await userEvent.setup().hover(create);
        expect(await screen.findByText("Changes are paused")).toBeInTheDocument();
    });
});
