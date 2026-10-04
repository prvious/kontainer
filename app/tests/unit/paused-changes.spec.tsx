import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, renderHook, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Trash2 } from "lucide-react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ActionButton } from "@/components/kit/action-button";
import { ConfirmDialog } from "@/components/kit/confirm-dialog";
import { RowActions } from "@/components/data-table/row-actions";
import { AuthContext, type AuthContextValue } from "@/contexts/auth-context-value";
import { ConnectivityProvider } from "@/contexts/connectivity-context";
import { ConnectivityContext, type ConnectivityContextValue } from "@/contexts/connectivity-context-value";
import { OperationsProvider } from "@/contexts/operations-context";
import { useCancelOperation } from "@/hooks/use-cancel-operation";
import { useConnectivity } from "@/hooks/use-connectivity";
import { useStartOperation, withIdempotencyKey } from "@/hooks/use-operations";

const offline: ConnectivityContextValue = { probe: { status: "unreachable" }, reachable: false, reconnectedAt: null, retry: () => {}, retrying: false };

function renderOffline(children: ReactNode, value: ConnectivityContextValue = offline) {
    return render(
        <TooltipProvider delay={0}>
            <ConnectivityContext value={value}>{children}</ConnectivityContext>
        </TooltipProvider>,
    );
}

describe("changes while the controller is unreachable", () => {
    it("restricts mutating buttons with the reason and leaves the rest alone", async () => {
        const onClick = vi.fn();
        renderOffline(
            <>
                <ActionButton mutates onClick={onClick}>
                    CREATE VOLUME
                </ActionButton>
                <ActionButton onClick={onClick}>REFRESH</ActionButton>
            </>,
        );
        const create = screen.getByRole("button", { name: /CREATE VOLUME/ });
        expect(create).toHaveAttribute("aria-disabled", "true");
        const user = userEvent.setup();
        await user.hover(create);
        expect(await screen.findByText("Changes are paused while offline")).toBeInTheDocument();
        await user.click(create);
        expect(onClick).not.toHaveBeenCalled();
        await user.click(screen.getByRole("button", { name: "REFRESH" }));
        expect(onClick).toHaveBeenCalledOnce();
    });

    it("keeps a confirm dialog from submitting, even with Enter", async () => {
        const onConfirm = vi.fn();
        renderOffline(<ConfirmDialog open onOpenChange={() => {}} title='Remove web?' confirmationText='web' confirmLabel='REMOVE' onConfirm={onConfirm} />);
        const user = userEvent.setup();
        await user.type(screen.getByLabelText("Type web to confirm"), "web{Enter}");
        expect(screen.getByRole("button", { name: /REMOVE/ })).toHaveAttribute("aria-disabled", "true");
        expect(onConfirm).not.toHaveBeenCalled();
    });

    it("disables mutating row actions and says why in the menu, leaving navigation alone", async () => {
        const remove = vi.fn();
        const show = vi.fn();
        renderOffline(
            <RowActions
                label='toolbox'
                actions={[{ label: "Delete", icon: Trash2, onClick: remove, mutates: true }]}
                menu={[
                    { label: "Show services", onClick: show },
                    { label: "Remove", onClick: remove, mutates: true },
                ]}
            />,
        );
        expect(screen.getByRole("button", { name: "Delete" })).toHaveAttribute("aria-disabled", "true");
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: "More actions for toolbox" }));
        expect(await screen.findByText("Changes are paused while offline")).toBeInTheDocument();
        expect(screen.getByRole("menuitem", { name: "Remove" })).toHaveAttribute("aria-disabled", "true");
        await user.click(screen.getByRole("menuitem", { name: "Show services" }));
        expect(show).toHaveBeenCalledOnce();
        expect(remove).not.toHaveBeenCalled();
    });

    it("changes nothing while the controller answers", () => {
        renderOffline(<ActionButton mutates>CREATE VOLUME</ActionButton>, { ...offline, reachable: true, probe: { status: "ready" } });
        expect(screen.getByRole("button", { name: /CREATE VOLUME/ })).not.toHaveAttribute("aria-disabled");
    });
});

describe("the shared change hooks while the controller is unreachable", () => {
    beforeEach(() => vi.clearAllMocks());

    it("refuse a start or a cancel with controller_unreachable instead of sending or parking it", async () => {
        const auth = { session: { profileId: "p1" }, expired: false, reportInventoryCustodyError: vi.fn() } as unknown as AuthContextValue;
        const queryClient = new QueryClient();
        const wrapper = ({ children }: { children: ReactNode }) => (
            <AuthContext value={auth}>
                <QueryClientProvider client={queryClient}>
                    <ConnectivityContext value={offline}>
                        <OperationsProvider>{children}</OperationsProvider>
                    </ConnectivityContext>
                </QueryClientProvider>
            </AuthContext>
        );
        const { result } = renderHook(
            () => ({
                start: useStartOperation<{ idempotencyKey: string }>({ command: "start_thing", scope: "services", title: () => "Thing", invalidate: [] }),
                cancel: useCancelOperation(),
            }),
            { wrapper },
        );
        const refusals: unknown[] = [];
        await act(() => result.current.start.mutateAsync(withIdempotencyKey({})).catch((code: unknown) => void refusals.push(code)));
        await act(() => result.current.cancel.mutateAsync("op-1").catch((code: unknown) => void refusals.push(code)));
        expect(refusals).toEqual(["controller_unreachable", "controller_unreachable"]);
        expect(invoke).not.toHaveBeenCalled();
        expect(
            queryClient
                .getMutationCache()
                .getAll()
                .some((mutation) => mutation.state.isPaused),
        ).toBe(false);
    });

    it("stays unreachable while a retried probe is out, until one answers", async () => {
        let probes = 0;
        let answer: (probe: unknown) => void = () => {};
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command !== "controller_health") return null;
            probes += 1;
            if (probes <= 3) return { status: "unreachable", diagnostic: "tcp" };
            return new Promise((resolve) => (answer = resolve));
        });
        function Reachability() {
            const { reachable, retry } = useConnectivity();
            return (
                <button type='button' onClick={retry}>
                    {reachable ? "reachable" : "unreachable"}
                </button>
            );
        }
        render(
            <QueryClientProvider client={new QueryClient()}>
                <ConnectivityProvider enabled>
                    <Reachability />
                </ConnectivityProvider>
            </QueryClientProvider>,
        );
        // Three attempts, 1 s apart, before the first verdict.
        await waitFor(() => expect(screen.getByRole("button")).toHaveTextContent("unreachable"), { timeout: 4000 });
        await act(async () => screen.getByRole("button").click());
        await waitFor(() => expect(probes).toBe(4));
        expect(screen.getByRole("button")).toHaveTextContent("unreachable");
        await act(async () => answer({ status: "ready" }));
        await waitFor(() => expect(screen.getByRole("button")).toHaveTextContent(/^reachable$/));
    }, 8000);
});
