import { useMutation } from "@tanstack/react-query";
import { invokeSession } from "./invoke-session";
import { sessionViewFromIpc } from "@/types/session";

export function useSessionLogin() {
    return useMutation({
        mutationFn: async ({ origin, label }: { origin: string; label: string | null }) =>
            sessionViewFromIpc(await invokeSession<unknown>("session_login", { origin, label })),
    });
}
