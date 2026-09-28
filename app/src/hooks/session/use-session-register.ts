import { useMutation } from "@tanstack/react-query";
import { invokeLogin } from "./invoke-login";
import { sessionViewFromIpc } from "@/types/session";

export interface RegisterOwnerVariables {
    origin: string;
    displayName: string;
    email: string;
    password: string;
    label: string | null;
}

export function useSessionRegister() {
    return useMutation({
        mutationFn: async (variables: RegisterOwnerVariables) => sessionViewFromIpc(await invokeLogin<unknown>("session_register", { ...variables })),
    });
}
