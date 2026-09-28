import { useMutation } from "@tanstack/react-query";
import { invokeSession } from "./invoke-session";

export function useSessionCancelLogin() {
    return useMutation({ mutationFn: () => invokeSession<void>("session_cancel_login") });
}
