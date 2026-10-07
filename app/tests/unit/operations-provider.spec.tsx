import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, renderHook, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { toast } from "sonner";
import { AuthContext, type AuthContextValue } from "@/contexts/auth-context-value";
import { OperationsProvider } from "@/contexts/operations-context";
import { PinnedOperations } from "@/components/operations/pinned-operations";
import { useOperation, operationKey } from "@/hooks/use-operation";
import { useOperations, useStartOperation, withIdempotencyKey } from "@/hooks/use-operations";
import fixture from "../fixtures/operations-ipc.json";

vi.mock("sonner", () => ({
    toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn(), warning: vi.fn(), loading: vi.fn(), dismiss: vi.fn() }),
}));

const profileId = "018f2e34-0000-7000-8000-0000000000bb";
const operationId = fixture.operation.id;
const auth = { session: { profileId }, expired: false, reportInventoryCustodyError: vi.fn() } as unknown as AuthContextValue;

function setup() {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const wrapper = ({ children }: { children: React.ReactNode }) => (
        <AuthContext value={auth}>
            <QueryClientProvider client={queryClient}>
                <OperationsProvider>{children}</OperationsProvider>
            </QueryClientProvider>
        </AuthContext>
    );
    return { queryClient, wrapper };
}

const succeeded = { ...fixture.operation, state: "succeeded", progress: { total: 3, done: 3 }, finishedAt: "2026-09-28T12:00:03.500Z" };

describe("operation tracking", () => {
    beforeEach(() => vi.clearAllMocks());

    it("polls every second and stops once the operation is terminal", async () => {
        const answers = [fixture.operation, fixture.operation, succeeded];
        vi.mocked(invoke).mockImplementation(async () => answers.shift() ?? succeeded);
        const { queryClient, wrapper } = setup();
        const { result } = renderHook(() => useOperation(operationId), { wrapper });
        await waitFor(() => expect(result.current.data?.state).toBe("succeeded"), { timeout: 4000 });
        const calls = vi.mocked(invoke).mock.calls.length;
        await new Promise((resolve) => setTimeout(resolve, 1300));
        expect(vi.mocked(invoke).mock.calls.length).toBe(calls);
        expect(invoke).toHaveBeenCalledWith("get_operation", { profileId, operationId });
        expect(queryClient.getQueryData(operationKey(profileId, operationId))).toMatchObject({ state: "succeeded" });
    }, 8000);

    it("starts with a key, tracks the operation, and on success invalidates and toasts once", async () => {
        const answers = [fixture.operation, succeeded, succeeded];
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "start_thing") return fixture.admitted;
            return answers.shift() ?? succeeded;
        });
        const { queryClient, wrapper } = setup();
        const invalidate = vi.spyOn(queryClient, "invalidateQueries");
        const { result } = renderHook(
            () => ({
                start: useStartOperation<{ name: string; idempotencyKey: string }>({
                    command: "start_thing",
                    scope: "services",
                    title: ({ name }) => `Restarting ${name}`,
                    invalidate: [["services", profileId]],
                }),
                operations: useOperations(),
            }),
            { wrapper },
        );
        await act(() => result.current.start.mutateAsync(withIdempotencyKey({ name: "web" })));
        expect(result.current.operations.tracked).toHaveLength(1);
        await waitFor(() => expect(toast.success).toHaveBeenCalledTimes(1), { timeout: 4000 });
        expect(toast.success).toHaveBeenCalledWith("Restarting web", expect.objectContaining({ description: "3 of 3 · finished in 3s" }));
        expect(invalidate).toHaveBeenCalledWith({ queryKey: ["services", profileId] });
        await waitFor(() => expect(result.current.operations.tracked).toHaveLength(0));
        await new Promise((resolve) => setTimeout(resolve, 50));
        expect(toast.success).toHaveBeenCalledTimes(1);
    }, 8000);

    it("reuses the idempotency key when a start is retried", async () => {
        let attempts = 0;
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "start_thing") {
                attempts += 1;
                if (attempts === 1) throw "controller_unavailable";
                return fixture.admitted;
            }
            return succeeded;
        });
        const { wrapper } = setup();
        const { result } = renderHook(
            () =>
                useStartOperation<{ idempotencyKey: string }>({
                    command: "start_thing",
                    scope: "services",
                    title: () => "Thing",
                    invalidate: [],
                }),
            { wrapper },
        );
        await act(() => result.current.mutateAsync(withIdempotencyKey({})));
        const keys = vi
            .mocked(invoke)
            .mock.calls.filter(([command]) => command === "start_thing")
            .map(([, args]) => (args as { idempotencyKey: string }).idempotencyKey);
        expect(keys).toHaveLength(2);
        expect(keys[0]).toBe(keys[1]);
        expect(keys[0]).toMatch(/^[0-9a-f-]{36}$/);
    });

    it("keeps a failed operation pinned with its reason until dismissed", async () => {
        vi.mocked(invoke).mockImplementation(async () => ({ ...fixture.failedOperation, id: operationId }));
        const { wrapper } = setup();
        function Harness() {
            const { track } = useOperations();
            return (
                <>
                    <button type='button' onClick={() => track({ id: operationId, scope: "services", title: "Scaling worker-processor", invalidate: [] })}>
                        track
                    </button>
                    <PinnedOperations scope='services' />
                    <PinnedOperations scope='containers' />
                </>
            );
        }
        render(<Harness />, { wrapper });
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: "track" }));
        expect(await screen.findByText("FAILED · no_eligible_nodes — No node satisfies the placement constraints. · req 01J9ZM2R")).toBeInTheDocument();
        await waitFor(() => expect(toast.error).toHaveBeenCalledTimes(1));
        expect(screen.getAllByRole("status")).toHaveLength(1);
        expect(screen.getByRole("status")).toHaveAttribute("data-phase", "failed");
        await new Promise((resolve) => setTimeout(resolve, 50));
        expect(screen.getByText("Scaling worker-processor")).toBeInTheDocument();
        await user.click(screen.getByRole("button", { name: "DISMISS" }));
        expect(screen.queryByText("Scaling worker-processor")).not.toBeInTheDocument();
    });

    it("says a status it couldn't read is unknown, without CANCEL, and stops polling once the operation is gone", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "get_operation") throw "not_found";
            return null;
        });
        const { wrapper } = setup();
        function Harness() {
            const { track } = useOperations();
            return (
                <>
                    <button type='button' onClick={() => track({ id: operationId, scope: "stacks", title: "Removing stack legacy-api", invalidate: [] })}>
                        track
                    </button>
                    <PinnedOperations scope='stacks' />
                </>
            );
        }
        render(<Harness />, { wrapper });
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: "track" }));
        const pinned = await screen.findByRole("region", { name: "Operations" });
        expect(await within(pinned).findByText("UNKNOWN · not_found — the status couldn't be read")).toBeInTheDocument();
        expect(within(pinned).getByRole("status")).toHaveAttribute("data-phase", "uncertain");
        expect(within(pinned).queryByRole("button", { name: "CANCEL" })).not.toBeInTheDocument();
        const polls = () => vi.mocked(invoke).mock.calls.filter(([command]) => command === "get_operation").length;
        const before = polls();
        await act(() => new Promise((resolve) => setTimeout(resolve, 1300)));
        expect(polls()).toBe(before);
        await user.click(within(pinned).getByRole("button", { name: "DISMISS" }));
        expect(screen.queryByText("Removing stack legacy-api")).not.toBeInTheDocument();
    });

    it("offers CANCEL while queued and cancels through Rust", async () => {
        const queued = { ...fixture.operation, state: "recoverable", progress: null, startedAt: null };
        const cancelled = { ...queued, state: "cancelled", finishedAt: "2026-09-28T12:00:02.000Z" };
        vi.mocked(invoke).mockImplementation(async (command: string) => (command === "cancel_operation" ? cancelled : queued));
        const { wrapper } = setup();
        function Harness() {
            const { track } = useOperations();
            return (
                <>
                    <button type='button' onClick={() => track({ id: operationId, scope: "stacks", title: "Removing stack legacy-api", invalidate: [] })}>
                        track
                    </button>
                    <PinnedOperations scope='stacks' />
                </>
            );
        }
        render(<Harness />, { wrapper });
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: "track" }));
        expect(await screen.findByText("QUEUED · waiting for a worker")).toBeInTheDocument();
        await user.click(screen.getByRole("button", { name: "CANCEL" }));
        expect(invoke).toHaveBeenCalledWith("cancel_operation", { profileId, operationId });
        await waitFor(() => expect(toast.warning).toHaveBeenCalledWith("Removing stack legacy-api cancelled", expect.anything()), { timeout: 3000 });
        await waitFor(() => expect(screen.queryByText("Removing stack legacy-api")).not.toBeInTheDocument());
    });
});
