import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import { useSettings } from "@/hooks/use-settings";
import type { NetworkItem } from "@/types/networks";

export function useNetworks() {
    const { agentUrl, token } = useAuth();
    const { refreshInterval } = useSettings();

    return useQuery({
        queryKey: ["networks"],
        queryFn: () => invoke<{ networks: NetworkItem[] }>("list_networks", { url: agentUrl, token }),
        enabled: !!agentUrl && !!token,
        refetchInterval: refreshInterval,
        select: (data) => data.networks,
    });
}
