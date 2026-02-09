import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import { useSettings } from "@/hooks/use-settings";
import type { ClusterImagesResponse } from "@/types/images";

export function useImages() {
    const { agentUrl, token } = useAuth();
    const { refreshInterval } = useSettings();

    return useQuery({
        queryKey: ["images"],
        queryFn: () =>
            invoke<ClusterImagesResponse>("list_images", {
                url: agentUrl,
                token,
            }),
        enabled: !!agentUrl && !!token,
        refetchInterval: refreshInterval,
    });
}
