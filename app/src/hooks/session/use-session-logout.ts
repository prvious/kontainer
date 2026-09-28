import { useMutation } from "@tanstack/react-query";
import { invokeSession } from "./invoke-session";

export function useSessionLogout() {
    return useMutation({ mutationFn: () => invokeSession<void>("session_logout") });
}
