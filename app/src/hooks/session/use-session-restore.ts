import { useQuery } from "@tanstack/react-query";
import { invokeSession } from "./invoke-session";
import { sessionViewFromIpc } from "@/types/session";

export const sessionRestoreKey = ["session", "restore"] as const;

export function useSessionRestore() {
    return useQuery({
        queryKey: sessionRestoreKey,
        queryFn: async () => {
            const value = await invokeSession<unknown>("session_restore");
            return value === null ? null : sessionViewFromIpc(value);
        },
        retry: false,
        staleTime: Infinity,
    });
}
