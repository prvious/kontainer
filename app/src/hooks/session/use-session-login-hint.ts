import { useQuery } from "@tanstack/react-query";
import { invokeSession } from "./invoke-session";
import { loginHintFromIpc } from "@/types/session";

export function useSessionLoginHint() {
    return useQuery({
        queryKey: ["session", "login-hint"],
        queryFn: async () => {
            const value = await invokeSession<unknown>("session_login_hint");
            return value === null ? null : loginHintFromIpc(value);
        },
        retry: false,
        staleTime: Infinity,
    });
}
