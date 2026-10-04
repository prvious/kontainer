import { useMutation } from "@tanstack/react-query";
import { invokeSession } from "./invoke-session";
import type { SessionView } from "@/types/session";

export function useSessionRetryCommit() {
    return useMutation({ mutationFn: () => invokeSession<SessionView>("session_retry_commit") });
}
