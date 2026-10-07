import { useQuery } from "@tanstack/react-query";
import { invokeSession } from "./invoke-session";
import type { SessionView } from "@/types/session";

export const sessionRestoreKey = ["session", "restore"] as const;

export function useSessionRestore() {
    return useQuery({
        queryKey: sessionRestoreKey,
        queryFn: () => invokeSession<SessionView | null>("session_restore"),
        retry: false,
        staleTime: Infinity,
    });
}
