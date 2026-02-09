import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import { useSettings } from "@/hooks/use-settings";
import type { VolumeItem } from "@/types/volumes";

export function useVolumes() {
    const { agentUrl, token } = useAuth();
    const { refreshInterval } = useSettings();

    return useQuery({
        queryKey: ["volumes"],
        queryFn: () => invoke<{ volumes: VolumeItem[] }>("list_volumes", { url: agentUrl, token }),
        enabled: !!agentUrl && !!token,
        refetchInterval: refreshInterval,
        select: (data) => data.volumes,
    });
}
