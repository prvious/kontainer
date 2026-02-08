import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";

interface UpdateServiceImageResponse {
    image: string;
}

export function useUpdateServiceImage(serviceId: string) {
    const { agentUrl, token } = useAuth();
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (image: string) =>
            invoke<UpdateServiceImageResponse>("update_service_image", {
                url: agentUrl,
                token,
                serviceId,
                image,
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["service", serviceId] });
            queryClient.invalidateQueries({ queryKey: ["services"] });
        },
    });
}
