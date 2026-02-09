import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import type { RemoveImageResponse } from "@/types/images";

export function useRemoveImage() {
    const { agentUrl, token } = useAuth();
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: ({ imageId, nodeId, force }: { imageId: string; nodeId: string; force: boolean }) =>
            invoke<RemoveImageResponse>("remove_image", {
                url: agentUrl,
                token,
                imageId,
                nodeId,
                force,
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["images"] });
        },
    });
}
