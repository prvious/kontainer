import { useMutation } from "@tanstack/react-query";
import { invokeLogin } from "./invoke-login";
import type { SessionView } from "@/types/session";

export interface PasswordLoginVariables {
    origin: string;
    email: string;
    password: string;
    label: string | null;
}

/**
 * The password travels only in this invoke; Rust zeroes it after the request. TanStack keeps
 * `variables` on the mutation, so `gcTime: 0` drops it from the cache as soon as the caller
 * `reset()`s the observer (the auth provider does, after every attempt).
 */
export function useSessionLogin() {
    return useMutation({
        mutationFn: (variables: PasswordLoginVariables) => invokeLogin<SessionView>("session_login", { ...variables }),
        gcTime: 0,
    });
}
