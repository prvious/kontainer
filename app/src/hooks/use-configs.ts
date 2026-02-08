import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import type { ConfigItem } from "@/types/configs";

export function useConfigs() {
    const { agentUrl, token } = useAuth();

    return useQuery({
        queryKey: ["configs"],
        queryFn: () => invoke<{ configs: ConfigItem[] }>("list_configs", { url: agentUrl, token }),
        enabled: !!agentUrl && !!token,
        refetchInterval: 5000,
        select: (data) => data.configs,
    });
}
