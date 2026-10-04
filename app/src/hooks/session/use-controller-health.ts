import { useQuery } from "@tanstack/react-query";
import { invokeSession } from "./invoke-session";
import type { ProbeView } from "@/types/session";

const controllerHealthKey = ["session", "health"] as const;
const HEALTH_INTERVAL_MS = 15_000;
/** Three unreachable attempts in a row, 1 s apart, mark the controller unreachable (spec 14). */
const PROBE_ATTEMPTS = 3;
const PROBE_RETRY_DELAY_MS = 1_000;

/**
 * The signed-in controller's `/health/ready`, every 15 s. An unreachable answer throws the probe,
 * so a fetch retries twice before the query errors; TanStack resets `failureCount` at the start
 * of every fetch, which is why "three failures" means three attempts of one fetch. The query stays
 * in error, even while the next fetch retries, until a probe answers.
 */
export function useControllerHealth(enabled = true) {
    return useQuery({
        queryKey: controllerHealthKey,
        enabled,
        refetchInterval: HEALTH_INTERVAL_MS,
        retry: PROBE_ATTEMPTS - 1,
        retryDelay: PROBE_RETRY_DELAY_MS,
        queryFn: async () => {
            const probe = await invokeSession<ProbeView | null>("controller_health");
            if (probe?.status === "unreachable") throw probe;
            return probe;
        },
    });
}
