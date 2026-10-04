import { useContext } from "react";
import { skipToken, useQueries, useQuery } from "@tanstack/react-query";
import { useAuth } from "@/hooks/use-auth";
import { useQueriesPaused } from "@/hooks/use-queries-paused";
import { authenticatedInvoke } from "@/hooks/authenticated-invoke";
import { OperationsContext, type TrackedOperation } from "@/contexts/operations-context-value";
import { isTerminal, type OperationState, type OperationView } from "@/types/operations";

const POLL_INTERVAL_MS = 1000;

export const operationKey = (profileId: string, operationId: string) => ["operations", profileId, operationId] as const;

/** Terminal, or uncertain (it will not move without someone looking): polling stops. */
export const isSettled = (state: OperationState) => isTerminal(state) || state === "uncertain";

/** Poll failures that asking again won't clear: polling stops and the row says so. */
export const isPermanentPollError = (error: unknown) => error === "not_found" || error === "forbidden" || error === "invalid_response";

/**
 * One operation, polled every second until it settles or a poll fails for good. Every settled
 * snapshot is handed to the operations provider, which acts on it once (invalidate, toast),
 * whichever observer fetched it.
 */
export function useOperation(operationId: string) {
    const { session, reportInventoryCustodyError } = useAuth();
    const paused = useQueriesPaused();
    const operations = useContext(OperationsContext);
    const profileId = session?.profileId ?? "";
    return useQuery({
        queryKey: operationKey(profileId, operationId),
        queryFn: async () => {
            const view = await authenticatedInvoke<OperationView>("get_operation", profileId, { operationId }, reportInventoryCustodyError);
            if (isSettled(view.state)) operations?.settle(view);
            return view;
        },
        enabled: Boolean(session) && !paused,
        refetchInterval: (query) =>
            (query.state.data && isSettled(query.state.data.state)) || isPermanentPollError(query.state.error) ? false : POLL_INTERVAL_MS,
        retry: false,
    });
}

/**
 * The tracked operations of one scope with their latest snapshots, read from the cache the
 * watchers keep fresh (these observers never fetch themselves).
 */
export function useTrackedOperations(scope: string): { operation: TrackedOperation; view: OperationView | undefined }[] {
    const operations = useContext(OperationsContext);
    const tracked = (operations?.tracked ?? []).filter((operation) => operation.scope === scope);
    const views = useOperationViews(tracked.map((operation) => operation.id));
    return tracked.map((operation, index) => ({ operation, view: views[index] }));
}

/**
 * The latest snapshots of these operations, read from the cache the provider's watchers keep
 * fresh; a settled one keeps its last snapshot after it leaves the tracked list.
 */
export function useOperationViews(operationIds: string[]): (OperationView | undefined)[] {
    const { session } = useAuth();
    const profileId = session?.profileId ?? "";
    const views = useQueries({
        queries: operationIds.map((operationId) => ({
            queryKey: operationKey(profileId, operationId),
            queryFn: skipToken,
        })),
    });
    return views.map((view) => view.data as OperationView | undefined);
}
