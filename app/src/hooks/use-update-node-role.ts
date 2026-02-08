import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";

interface UpdateNodeRoleResponse {
    role: string;
}

export function useUpdateNodeRole(nodeId: string) {
    const { agentUrl, token } = useAuth();
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (role: string) =>
            invoke<UpdateNodeRoleResponse>("update_node_role", {
                url: agentUrl,
                token,
                nodeId,
                role,
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["nodes"] });
        },
    });
}
