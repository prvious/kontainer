import { keepPreviousData, skipToken, useQuery } from "@tanstack/react-query";
import { useAuth } from "@/hooks/use-auth";
import { useAppSettings } from "@/hooks/use-app-settings";
import { useQueriesPaused } from "@/hooks/use-queries-paused";
import { authenticatedInvoke } from "@/hooks/authenticated-invoke";
import { useStartOperation, type WithIdempotencyKey } from "@/hooks/use-operations";
import { servicesKey } from "@/hooks/use-services";
import { stackListFromIpc, stackPlanFromIpc } from "@/types/stacks";
import type { OperationView } from "@/types/operations";
import { DEFAULT_APP_SETTINGS } from "@/types/settings";

export const stacksKey = (profileId: string) => ["stacks", profileId] as const;

/** Every stack, polled at the refresh interval (every 10 s while it errors). */
export function useStacks() {
    const { session, reportInventoryCustodyError } = useAuth();
    const paused = useQueriesPaused();
    const settings = useAppSettings();
    const profileId = session?.profileId ?? "";
    const interval = settings.data?.refreshInterval ?? DEFAULT_APP_SETTINGS.refreshInterval ?? 5000;
    return useQuery({
        queryKey: stacksKey(profileId),
        queryFn: () => authenticatedInvoke("list_stacks", profileId, {}, stackListFromIpc, reportInventoryCustodyError),
        enabled: Boolean(session) && !paused,
        refetchInterval: (query) => (query.state.status === "error" ? 10_000 : interval),
        retry: false,
        placeholderData: keepPreviousData,
    });
}

/** The removal plan, fetched while the remove dialog is open for `name`. */
export function useStackPlan(name: string | null) {
    const { session, reportInventoryCustodyError } = useAuth();
    const profileId = session?.profileId ?? "";
    return useQuery({
        queryKey: [...stacksKey(profileId), "plan", name] as const,
        queryFn:
            name && session
                ? () => authenticatedInvoke("get_stack_removal_plan", profileId, { name }, stackPlanFromIpc, reportInventoryCustodyError)
                : skipToken,
        retry: false,
        staleTime: 0,
    });
}

export interface RemoveStackVariables extends WithIdempotencyKey {
    name: string;
}

/** Remove one stack; `onSettled` lets the dialog close on success. */
export function useRemoveStack(onSettled?: (view: OperationView, variables: RemoveStackVariables) => void) {
    const { session } = useAuth();
    const profileId = session?.profileId ?? "";
    return useStartOperation<RemoveStackVariables>({
        command: "remove_stack",
        scope: "stacks",
        title: ({ name }) => `Removing stack ${name}`,
        success: ({ name }) => ({ title: `Removed stack ${name}` }),
        targets: ({ name }) => [name],
        invalidate: [stacksKey(profileId), servicesKey(profileId)],
        args: ({ name, idempotencyKey }) => ({ name, idempotencyKey }),
        onSettled,
    });
}
