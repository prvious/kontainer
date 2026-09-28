import { useMutation } from "@tanstack/react-query";
import { invokeSession } from "./invoke-session";
import { sessionViewFromIpc } from "@/types/session";

export function useSessionRetryCommit() {
    return useMutation({ mutationFn: async () => sessionViewFromIpc(await invokeSession<unknown>("session_retry_commit")) });
}
