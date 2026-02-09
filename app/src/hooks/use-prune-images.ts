import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import type { PruneImagesResponse } from "@/types/images";

export function usePruneImages() {
    const { agentUrl, token } = useAuth();
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: () =>
            invoke<PruneImagesResponse>("prune_images", {
                url: agentUrl,
                token,
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["images"] });
        },
    });
}
