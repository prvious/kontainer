import { useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import { configuredNodeFromIpc, containerPageFromIpc, inventoryErrorCode, type InventoryErrorCode } from "@/types/inventory";

export const configuredNodeKey = (profileId: string | null) => ["configured-node", profileId] as const;
export const containerInventoryKey = (profileId: string | null, nodeId: string | null, cursor: string | null, limit: number) =>
    ["container-inventory", profileId, nodeId, cursor, limit] as const;

function reportError(profileId: string, error: unknown, report: ((profileId: string, code: InventoryErrorCode) => void) | undefined): InventoryErrorCode {
    const code = inventoryErrorCode(error);
    report?.(profileId, code);
    return code;
}

export function useConfiguredNode(profileId: string | null) {
    const { reportInventoryCustodyError } = useAuth();
    return useQuery({
        queryKey: configuredNodeKey(profileId),
        enabled: profileId !== null,
        retry: false,
        queryFn: async () => {
            if (!profileId) throw "unauthenticated";
            try {
                return configuredNodeFromIpc(await invoke<unknown>("get_configured_node", { profileId }));
            } catch (error) {
                throw reportError(profileId, error, reportInventoryCustodyError);
            }
        },
    });
}

export function useContainerInventory(profileId: string, nodeId: string, cursor: string | null, limit: number) {
    const { reportInventoryCustodyError } = useAuth();
    const queryClient = useQueryClient();
    return useQuery({
        queryKey: containerInventoryKey(profileId, nodeId, cursor, limit),
        retry: false,
        queryFn: async () => {
            try {
                return containerPageFromIpc(await invoke<unknown>("list_container_inventory", { profileId, nodeId, cursor, limit }));
            } catch (error) {
                const code = reportError(profileId, error, reportInventoryCustodyError);
                if (code === "node_not_found") await queryClient.invalidateQueries({ queryKey: configuredNodeKey(profileId) });
                throw code;
            }
        },
    });
}
