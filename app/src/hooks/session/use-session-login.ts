import { useMutation } from "@tanstack/react-query";
import { invokeLogin } from "./invoke-login";
import { sessionViewFromIpc } from "@/types/session";

export interface PasswordLoginVariables {
    origin: string;
    email: string;
    password: string;
    label: string | null;
}

/** The password travels only in this invoke; Rust zeroes it after the request. */
export function useSessionLogin() {
    return useMutation({
        mutationFn: async (variables: PasswordLoginVariables) => sessionViewFromIpc(await invokeLogin<unknown>("session_login", { ...variables })),
    });
}
