import { useQuery } from "@tanstack/react-query";
import { invokeSession } from "./invoke-session";
import { probeViewFromIpc } from "@/types/session";

export const controllerHealthKey = ["session", "health"] as const;
export const HEALTH_INTERVAL_MS = 15_000;

/**
 * The signed-in controller's `/health/ready`, every 15 s. `failureCount` counts
 * consecutive unreachable probes (spec 14 treats three as unreachable).
 */
export function useControllerHealth(enabled = true) {
    return useQuery({
        queryKey: controllerHealthKey,
        enabled,
        refetchInterval: HEALTH_INTERVAL_MS,
        retry: false,
        queryFn: async () => {
            const value = await invokeSession<unknown>("controller_health");
            return value === null ? null : probeViewFromIpc(value);
        },
    });
}
