import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import type { SecretItem } from "@/types/secrets";

export function useSecrets() {
    const { agentUrl, token } = useAuth();

    return useQuery({
        queryKey: ["secrets"],
        queryFn: () => invoke<{ secrets: SecretItem[] }>("list_secrets", { url: agentUrl, token }),
        enabled: !!agentUrl && !!token,
        refetchInterval: 5000,
        select: (data) => data.secrets,
    });
}
