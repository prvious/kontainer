import { useContext } from "react";
import { useQuery } from "@tanstack/react-query";
import { useAuth } from "@/hooks/use-auth";
import { useQueriesPaused } from "@/hooks/use-queries-paused";
import { authenticatedInvoke } from "@/hooks/authenticated-invoke";
import { OperationsContext } from "@/contexts/operations-context-value";
import { isTerminal, operationFromIpc, type OperationState } from "@/types/operations";

export const POLL_INTERVAL_MS = 1000;

export const operationKey = (profileId: string, operationId: string) => ["operations", profileId, operationId] as const;

/** Terminal, or uncertain (it will not move without someone looking): polling stops. */
export const isSettled = (state: OperationState) => isTerminal(state) || state === "uncertain";

/**
 * One operation, polled every second until it settles. Every settled snapshot is handed to the
 * operations provider, which acts on it once (invalidate, toast), whichever observer fetched it.
 */
export function useOperation(operationId: string) {
    const { session, reportInventoryCustodyError } = useAuth();
    const paused = useQueriesPaused();
    const operations = useContext(OperationsContext);
    const profileId = session?.profileId ?? "";
    return useQuery({
        queryKey: operationKey(profileId, operationId),
        queryFn: async () => {
            const view = await authenticatedInvoke("get_operation", profileId, { operationId }, operationFromIpc, reportInventoryCustodyError);
            if (isSettled(view.state)) operations?.settle(view);
            return view;
        },
        enabled: Boolean(session) && !paused,
        refetchInterval: (query) => (query.state.data && isSettled(query.state.data.state) ? false : POLL_INTERVAL_MS),
        retry: false,
    });
}
