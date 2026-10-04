import { keepPreviousData, useQuery, useQueryClient } from "@tanstack/react-query";
import { useAuth } from "@/hooks/use-auth";
import { useAppSettings } from "@/hooks/use-app-settings";
import { useQueriesPaused } from "@/hooks/use-queries-paused";
import { authenticatedInvoke } from "@/hooks/authenticated-invoke";
import type { OperationView } from "@/types/operations";
import { useStartOperation, type WithIdempotencyKey } from "@/hooks/use-operations";
import { containerRowId, type ContainerAction, type ContainerListView, type ContainerView } from "@/types/containers";
import { DEFAULT_APP_SETTINGS } from "@/types/settings";

export const containersKey = (profileId: string) => ["containers", profileId] as const;

/** A row the latest snapshot did not confirm: its node did not answer, so it is last known. */
export interface ContainerRow extends ContainerView {
    stale: boolean;
}

export interface ContainerSnapshot extends Omit<ContainerListView, "items"> {
    items: ContainerRow[];
}

/**
 * Keep the last good rows of a node that did not answer this time, marked stale, so a silent
 * node's containers stay visible as UNKNOWN (03.S4). The query function merges with the previous
 * snapshot when data arrives, never in an effect.
 */
export function mergeSnapshots(previous: ContainerSnapshot | undefined, next: ContainerListView): ContainerSnapshot {
    const silent = new Set(next.nodes.filter((node) => node.status !== "ok").map((node) => node.nodeId));
    const carried = (previous?.items ?? []).filter((row) => silent.has(row.nodeId)).map((row) => ({ ...row, stale: true }));
    return { ...next, items: [...next.items.map((item) => ({ ...item, stale: false })), ...carried] };
}

/** Every container on the cluster, polled at the refresh interval. */
export function useContainers() {
    const { session, reportInventoryCustodyError } = useAuth();
    const paused = useQueriesPaused();
    const settings = useAppSettings();
    const queryClient = useQueryClient();
    const profileId = session?.profileId ?? "";
    return useQuery({
        queryKey: containersKey(profileId),
        queryFn: async (): Promise<ContainerSnapshot> => {
            const next = await authenticatedInvoke<ContainerListView>("list_containers", profileId, { nodeId: null }, reportInventoryCustodyError);
            return mergeSnapshots(queryClient.getQueryData<ContainerSnapshot>(containersKey(profileId)), next);
        },
        enabled: Boolean(session) && !paused,
        refetchInterval: settings.data?.refreshInterval ?? DEFAULT_APP_SETTINGS.refreshInterval ?? 5000,
        retry: false,
        placeholderData: keepPreviousData,
    });
}

const PRESENT: Record<ContainerAction, string> = {
    start: "Starting",
    stop: "Stopping",
    restart: "Restarting",
    pause: "Pausing",
    unpause: "Unpausing",
    remove: "Removing",
};

export interface ContainerActionVariables extends WithIdempotencyKey {
    nodeId: string;
    containerId: string;
    action: ContainerAction;
    options: { force?: boolean; timeoutSeconds?: number } | null;
    /** For the operation title only; not sent. */
    name: string;
}

/** One lifecycle action, tracked under the containers scope. */
export function useContainerAction(onSettled?: (view: OperationView, variables: ContainerActionVariables) => void) {
    const { session } = useAuth();
    const profileId = session?.profileId ?? "";
    return useStartOperation<ContainerActionVariables>({
        command: "container_action",
        scope: "containers",
        title: ({ action, name }) => `${PRESENT[action]} ${name}`,
        success: ({ action, name }) =>
            action === "remove" ? { title: `Removed ${name}`, description: "Removal is final; the container and its writable layer are gone." } : undefined,
        targets: (variables) => [containerRowId(variables)],
        invalidate: [containersKey(profileId)],
        args: ({ nodeId, containerId, action, options, idempotencyKey }) => ({ nodeId, containerId, action, options, idempotencyKey }),
        onSettled,
    });
}

export interface BulkActionVariables extends WithIdempotencyKey {
    action: ContainerAction;
    targets: { nodeId: string; containerId: string }[];
    force: boolean;
}

/** One action over the selection as a single `containers.bulk` operation. */
export function useContainersBulkAction() {
    const { session } = useAuth();
    const profileId = session?.profileId ?? "";
    return useStartOperation<BulkActionVariables>({
        command: "containers_bulk_action",
        scope: "containers",
        title: ({ action, targets }) => `${PRESENT[action]} ${targets.length} ${targets.length === 1 ? "container" : "containers"}`,
        targets: (variables: BulkActionVariables) => variables.targets.map((target) => containerRowId(target)),
        invalidate: [containersKey(profileId)],
        args: ({ action, targets, force, idempotencyKey }: BulkActionVariables) => ({ action, targets, force, idempotencyKey }),
    });
}
