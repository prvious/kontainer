import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import { useSettings } from "@/hooks/use-settings";
import type { StackItem } from "@/types/stacks";

export function useStacks() {
    const { agentUrl, token } = useAuth();
    const { refreshInterval } = useSettings();

    return useQuery({
        queryKey: ["stacks"],
        queryFn: () => invoke<{ stacks: StackItem[] }>("list_stacks", { url: agentUrl, token }),
        enabled: !!agentUrl && !!token,
        refetchInterval: refreshInterval,
        select: (data) => data.stacks,
    });
}
