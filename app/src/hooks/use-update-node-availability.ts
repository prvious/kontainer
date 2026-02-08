import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";

interface UpdateNodeAvailabilityResponse {
    availability: string;
}

export function useUpdateNodeAvailability(nodeId: string) {
    const { agentUrl, token } = useAuth();
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (availability: string) =>
            invoke<UpdateNodeAvailabilityResponse>("update_node_availability", {
                url: agentUrl,
                token,
                nodeId,
                availability,
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["nodes"] });
        },
    });
}
