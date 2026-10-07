import { useMutation } from "@tanstack/react-query";
import { invokeSession } from "./invoke-session";
import type { ConnectionSummary } from "@/types/session";

/** Sign out; `summary` is cached on the saved connection for the switcher. */
export function useSessionLogout() {
    return useMutation({
        mutationFn: (summary?: ConnectionSummary) => invokeSession<void>("session_logout", { summary: summary ?? null }),
    });
}
