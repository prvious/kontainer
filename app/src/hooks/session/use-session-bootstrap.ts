import { useQuery } from "@tanstack/react-query";
import { invokeLogin } from "./invoke-login";

export const sessionBootstrapKey = (origin: string) => ["session", "bootstrap", origin] as const;

/** Whether the controller at `origin` has its owner; `null` origin disables the check. */
export function useSessionBootstrap(origin: string | null) {
    return useQuery({
        queryKey: sessionBootstrapKey(origin ?? ""),
        enabled: origin !== null,
        retry: false,
        staleTime: 30_000,
        queryFn: async () => (await invokeLogin<{ ownerExists: boolean }>("session_bootstrap", { origin })).ownerExists,
    });
}
