import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { invokeSession } from "./invoke-session";
import type { ProbeView, SavedConnection } from "@/types/session";

export const sessionConnectionsKey = ["session", "connections"] as const;

export function useSessionConnections() {
    return useQuery({
        queryKey: sessionConnectionsKey,
        queryFn: () => invokeSession<SavedConnection[]>("session_connections"),
        staleTime: Infinity,
    });
}

export function useForgetConnection() {
    const queryClient = useQueryClient();
    return useMutation({
        mutationFn: (id: string) => invokeSession<void>("session_forget_connection", { id }),
        onSuccess: () => queryClient.invalidateQueries({ queryKey: sessionConnectionsKey }),
    });
}

const MAX_PROBES_IN_FLIGHT = 3;

/** Probe every origin once, at most three at a time; one origin's failure never stops the rest. */
export async function probeAll(origins: string[]): Promise<Record<string, ProbeView>> {
    const results: Record<string, ProbeView> = {};
    let next = 0;
    async function worker() {
        while (next < origins.length) {
            const origin = origins[next++];
            try {
                results[origin] = await invoke<ProbeView>("connection_probe", { origin });
            } catch {
                results[origin] = { status: "unreachable" };
            }
        }
    }
    await Promise.all(Array.from({ length: Math.min(MAX_PROBES_IN_FLIGHT, origins.length) }, worker));
    return results;
}

/** Status dots for saved connections: probed once per set of origins. */
export function useConnectionProbes(origins: string[]) {
    return useQuery({
        queryKey: ["session", "probes", origins] as const,
        enabled: origins.length > 0,
        staleTime: Infinity,
        queryFn: () => probeAll(origins),
    });
}

/** A single probe for the connect card's diagnostics line. */
export function useConnectionProbe(origin: string | null) {
    return useQuery({
        queryKey: ["session", "probe", origin ?? ""] as const,
        enabled: origin !== null,
        retry: false,
        staleTime: 0,
        queryFn: () => invokeSession<ProbeView>("connection_probe", { origin }),
    });
}
