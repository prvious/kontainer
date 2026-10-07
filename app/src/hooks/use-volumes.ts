import { skipToken, useQuery } from "@tanstack/react-query";
import { useAuth } from "@/hooks/use-auth";
import { useAppSettings } from "@/hooks/use-app-settings";
import { useQueriesPaused } from "@/hooks/use-queries-paused";
import { authenticatedInvoke } from "@/hooks/authenticated-invoke";
import { useStartOperation, type WithIdempotencyKey } from "@/hooks/use-operations";
import { volumeRowId, VOLUME_NAME, type VolumeListView } from "@/types/volumes";
import type { OperationView } from "@/types/operations";
import { DEFAULT_APP_SETTINGS } from "@/types/settings";

const volumesKey = (profileId: string) => ["volumes", profileId] as const;

/** Every volume on the cluster, one row per node and name, polled at the refresh interval. */
export function useVolumes() {
    const { session, reportInventoryCustodyError } = useAuth();
    const paused = useQueriesPaused();
    const settings = useAppSettings();
    const profileId = session?.profileId ?? "";
    return useQuery({
        queryKey: volumesKey(profileId),
        queryFn: () => authenticatedInvoke<VolumeListView>("list_volumes", profileId, { nodeId: null, name: null }, reportInventoryCustodyError),
        enabled: Boolean(session) && !paused,
        refetchInterval: settings.data?.refreshInterval ?? DEFAULT_APP_SETTINGS.refreshInterval ?? 5000,
        retry: false,
    });
}

/**
 * Whether `name` is already taken on `nodeId`, asked of the controller (the create dialog's
 * check). Runs only for a well-formed name on a chosen node; `data` is `true` when taken.
 */
export function useVolumeNameTaken(nodeId: string | null, name: string | null) {
    const { session, reportInventoryCustodyError } = useAuth();
    const profileId = session?.profileId ?? "";
    const ready = Boolean(session) && nodeId !== null && name !== null && VOLUME_NAME.test(name);
    return useQuery({
        queryKey: [...volumesKey(profileId), "conflict", nodeId, name] as const,
        queryFn: ready
            ? async () => {
                  const list = await authenticatedInvoke<VolumeListView>("list_volumes", profileId, { nodeId, name }, reportInventoryCustodyError);
                  return list.items.length > 0;
              }
            : skipToken,
        retry: false,
        staleTime: 0,
    });
}

export interface CreateVolumeVariables extends WithIdempotencyKey {
    nodeId: string;
    name: string;
    labels: Record<string, string>;
}

/** One volume create, tracked under the volumes scope. */
export function useCreateVolume(onSettled?: (view: OperationView, variables: CreateVolumeVariables) => void) {
    const { session } = useAuth();
    const profileId = session?.profileId ?? "";
    return useStartOperation<CreateVolumeVariables>({
        command: "create_volume",
        scope: "volumes",
        title: ({ name, nodeId }) => `Creating ${name} on ${nodeId}`,
        success: ({ name, nodeId }) => ({ title: `Created ${name}`, description: `The volume is ready on ${nodeId}.` }),
        invalidate: [volumesKey(profileId)],
        args: ({ nodeId, name, labels, idempotencyKey }) => ({ volume: { nodeId, name, labels }, idempotencyKey }),
        onSettled,
    });
}

export interface RemoveVolumeVariables extends WithIdempotencyKey {
    nodeId: string;
    name: string;
}

/** One volume removal, tracked under the volumes scope; the row fades while it runs. */
export function useRemoveVolume() {
    const { session } = useAuth();
    const profileId = session?.profileId ?? "";
    return useStartOperation<RemoveVolumeVariables>({
        command: "remove_volume",
        scope: "volumes",
        title: ({ name, nodeId }) => `Removing ${name} from ${nodeId}`,
        success: ({ name }) => ({ title: `Removed ${name}`, description: "Its data is gone; removal can't be undone." }),
        targets: (variables) => [volumeRowId(variables)],
        invalidate: [volumesKey(profileId)],
        args: ({ nodeId, name, idempotencyKey }) => ({ nodeId, name, idempotencyKey }),
    });
}
