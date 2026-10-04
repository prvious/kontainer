import { skipToken, useQuery } from "@tanstack/react-query";
import { useAuth } from "@/hooks/use-auth";
import { useAppSettings } from "@/hooks/use-app-settings";
import { useQueriesPaused } from "@/hooks/use-queries-paused";
import { authenticatedInvoke } from "@/hooks/authenticated-invoke";
import { useStartOperation, type WithIdempotencyKey } from "@/hooks/use-operations";
import { containersKey } from "@/hooks/use-containers";
import { networkRowId, type AttachedContainerView, type NetworkListView, type NetworkScope } from "@/types/networks";
import type { OperationView } from "@/types/operations";
import { DEFAULT_APP_SETTINGS } from "@/types/settings";

const networksKey = (profileId: string) => ["networks", profileId] as const;

/** Every network on the cluster, polled at the refresh interval. */
export function useNetworks() {
    const { session, reportInventoryCustodyError } = useAuth();
    const paused = useQueriesPaused();
    const settings = useAppSettings();
    const profileId = session?.profileId ?? "";
    return useQuery({
        queryKey: networksKey(profileId),
        queryFn: () => authenticatedInvoke<NetworkListView>("list_networks", profileId, {}, reportInventoryCustodyError),
        enabled: Boolean(session) && !paused,
        refetchInterval: settings.data?.refreshInterval ?? DEFAULT_APP_SETTINGS.refreshInterval ?? 5000,
        retry: false,
    });
}

/**
 * The containers attached to one network, fetched only while `target` is set (the blocked-remove
 * popover sets it when it opens).
 */
export function useNetworkContainers(target: { id: string; nodeId: string | null } | null) {
    const { session, reportInventoryCustodyError } = useAuth();
    const profileId = session?.profileId ?? "";
    return useQuery({
        queryKey: [...networksKey(profileId), "containers", target?.id ?? null, target?.nodeId ?? null] as const,
        queryFn:
            target && session
                ? () =>
                      authenticatedInvoke<AttachedContainerView[]>(
                          "inspect_network",
                          profileId,
                          { networkId: target.id, nodeId: target.nodeId },
                          reportInventoryCustodyError,
                      )
                : skipToken,
        retry: false,
        staleTime: 0,
    });
}

export interface CreateNetworkVariables extends WithIdempotencyKey {
    name: string;
    driver: "overlay" | "bridge";
    attachable: boolean;
    nodeId: string | null;
    subnet: string | null;
}

/** One network create, tracked under the networks scope. */
export function useCreateNetwork(onSettled?: (view: OperationView, variables: CreateNetworkVariables) => void) {
    const { session } = useAuth();
    const profileId = session?.profileId ?? "";
    return useStartOperation<CreateNetworkVariables>({
        command: "create_network",
        scope: "networks",
        title: ({ name }) => `Creating ${name}`,
        success: ({ name, driver, nodeId }) => ({
            title: `Created ${name}`,
            description: driver === "overlay" ? "The overlay spans the swarm." : `The bridge is ready on ${nodeId}.`,
        }),
        invalidate: [networksKey(profileId)],
        args: ({ name, driver, attachable, nodeId, subnet, idempotencyKey }) => ({
            network: { name, driver, attachable: driver === "overlay" && attachable, nodeId, subnet },
            idempotencyKey,
        }),
        onSettled,
    });
}

export interface RemoveNetworkVariables extends WithIdempotencyKey {
    id: string;
    scope: NetworkScope;
    nodeIds: string[];
    name: string;
}

/** One network removal, tracked under the networks scope; the row fades while it runs. */
export function useRemoveNetwork() {
    const { session } = useAuth();
    const profileId = session?.profileId ?? "";
    return useStartOperation<RemoveNetworkVariables>({
        command: "remove_network",
        scope: "networks",
        title: ({ name }) => `Removing ${name}`,
        success: ({ name }) => ({ title: `Removed ${name}` }),
        targets: (variables) => [networkRowId(variables)],
        invalidate: [networksKey(profileId), containersKey(profileId)],
        args: ({ id, scope, nodeIds, idempotencyKey }) => ({ networkId: id, nodeId: scope === "local" ? (nodeIds[0] ?? null) : null, idempotencyKey }),
    });
}
