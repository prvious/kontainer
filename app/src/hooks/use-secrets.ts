import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import { useSettings } from "@/hooks/use-settings";
import type { SecretItem } from "@/types/secrets";

export function useSecrets() {
    const { agentUrl, token } = useAuth();
    const { refreshInterval } = useSettings();

    return useQuery({
        queryKey: ["secrets"],
        queryFn: () => invoke<{ secrets: SecretItem[] }>("list_secrets", { url: agentUrl, token }),
        enabled: !!agentUrl && !!token,
        refetchInterval: refreshInterval,
        select: (data) => data.secrets,
    });
}
