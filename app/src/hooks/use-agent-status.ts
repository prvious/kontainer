import { useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";

const POLL_INTERVAL = 15_000;

export function useAgentStatus(agentUrl: string | null, token: string | null, onAuthFailure: () => void) {
    const onAuthFailureRef = useRef(onAuthFailure);
    onAuthFailureRef.current = onAuthFailure;

    const query = useQuery({
        queryKey: ["agent-status", agentUrl],
        queryFn: () => invoke("check_auth", { url: agentUrl, token }),
        enabled: !!agentUrl && !!token,
        refetchInterval: POLL_INTERVAL,
        retry: false,
    });

    useEffect(() => {
        if (query.error && String(query.error) === "401") {
            onAuthFailureRef.current();
        }
    }, [query.error]);

    return query;
}
