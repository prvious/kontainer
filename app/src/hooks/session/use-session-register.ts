import { useMutation } from "@tanstack/react-query";
import { invokeLogin } from "./invoke-login";
import { sessionViewFromIpc } from "@/types/session";

export interface RegisterOwnerVariables {
    origin: string;
    displayName: string;
    email: string;
    password: string;
    /** The one-time code the controller prints at startup; handled like the password. */
    setupCode: string;
    label: string | null;
}

/** The password and setup code are held exactly as in `useSessionLogin`: never cached past the attempt. */
export function useSessionRegister() {
    return useMutation({
        mutationFn: async (variables: RegisterOwnerVariables) => sessionViewFromIpc(await invokeLogin<unknown>("session_register", { ...variables })),
        gcTime: 0,
    });
}
