import { useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import { useQueriesPaused } from "@/hooks/use-queries-paused";
import { nodesQueries } from "@/hooks/use-nodes";
import { containerPageFromIpc, inventoryErrorCode, type InventoryErrorCode } from "@/types/inventory";

export const containerInventoryKey = (profileId: string | null, nodeId: string | null, cursor: string | null, limit: number) =>
    ["container-inventory", profileId, nodeId, cursor, limit] as const;

function reportError(profileId: string, error: unknown, report: ((profileId: string, code: InventoryErrorCode) => void) | undefined): InventoryErrorCode {
    const code = inventoryErrorCode(error);
    report?.(profileId, code);
    return code;
}

export function useContainerInventory(profileId: string, nodeId: string, cursor: string | null, limit: number) {
    const { reportInventoryCustodyError } = useAuth();
    const paused = useQueriesPaused();
    const queryClient = useQueryClient();
    return useQuery({
        queryKey: containerInventoryKey(profileId, nodeId, cursor, limit),
        enabled: !paused,
        retry: false,
        queryFn: async () => {
            try {
                return containerPageFromIpc(await invoke<unknown>("list_container_inventory", { profileId, nodeId, cursor, limit }));
            } catch (error) {
                const code = reportError(profileId, error, reportInventoryCustodyError);
                if (code === "node_not_found") await queryClient.invalidateQueries({ queryKey: nodesQueries.all(profileId) });
                throw code;
            }
        },
    });
}
