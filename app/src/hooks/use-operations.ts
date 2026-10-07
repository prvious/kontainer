import { useContext } from "react";
import { useMutation, type QueryKey } from "@tanstack/react-query";
import { useAuth } from "@/hooks/use-auth";
import { useConnectivity } from "@/hooks/use-connectivity";
import { authenticatedInvoke } from "@/hooks/authenticated-invoke";
import { CONTROLLER_UNREACHABLE } from "@/contexts/connectivity-context-value";
import { OperationsContext, type OperationsContextValue } from "@/contexts/operations-context-value";
import type { AdmittedOperation, OperationView } from "@/types/operations";

export function useOperations(): OperationsContextValue {
    const context = useContext(OperationsContext);
    if (!context) throw new Error("useOperations must be used inside OperationsProvider");
    return context;
}

/** Mutation variables always carry the key of the gesture that started them. */
export interface WithIdempotencyKey {
    idempotencyKey: string;
}

/** One key per user gesture; pass the result to `mutate` so a retry reuses it. */
export function withIdempotencyKey<V extends object>(variables: V): V & WithIdempotencyKey {
    return { ...variables, idempotencyKey: crypto.randomUUID() };
}

/**
 * Start a mutation through a Tauri command that returns `{ operationId, replayed }`, then track
 * the operation. A transport failure retries once with the same variables, hence the same key,
 * so the controller replays instead of admitting a second operation. While the controller is
 * unreachable the change is refused with `controller_unreachable` and never sent, whatever
 * started it (03.S2).
 */
export function useStartOperation<V extends WithIdempotencyKey>({
    command,
    scope,
    title,
    invalidate,
    success,
    targets,
    args = (variables) => ({ ...variables }) as Record<string, unknown>,
    onSettled,
}: {
    command: string;
    scope: string;
    title: (variables: V) => string;
    invalidate: QueryKey[];
    /** Success toast text when it should differ from the title. */
    success?: (variables: V) => { title: string; description?: string } | undefined;
    /** Row ids the operation acts on. */
    targets?: (variables: V) => string[];
    /** The command's arguments; defaults to the variables themselves. */
    args?: (variables: V) => Record<string, unknown>;
    /** Called once when the operation settles, with the variables that started it. */
    onSettled?: (view: OperationView, variables: V) => void;
}) {
    const { session, reportInventoryCustodyError } = useAuth();
    const { track } = useOperations();
    const { reachable } = useConnectivity();
    const profileId = session?.profileId ?? "";
    return useMutation({
        mutationFn: async (variables: V) => {
            if (!reachable) throw CONTROLLER_UNREACHABLE;
            return authenticatedInvoke<AdmittedOperation>(command, profileId, args(variables), reportInventoryCustodyError);
        },
        retry: (failures, error: unknown) => failures < 1 && error === "controller_unavailable",
        onSuccess: (admitted, variables) =>
            track({
                id: admitted.operationId,
                scope,
                title: title(variables),
                invalidate,
                success: success?.(variables),
                targets: targets?.(variables),
                onSettled: onSettled && ((view) => onSettled(view, variables)),
            }),
    });
}
