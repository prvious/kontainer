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
        queryFn: async () => {
            const value = await invokeLogin<{ ownerExists?: unknown }>("session_bootstrap", { origin });
            if (typeof value?.ownerExists !== "boolean") throw { code: "invalid_response" };
            return value.ownerExists;
        },
    });
}
