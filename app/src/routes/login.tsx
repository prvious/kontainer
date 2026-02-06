import { createFileRoute, redirect } from "@tanstack/react-router";
import { LoginScreen } from "@/components/login-screen";

export const Route = createFileRoute("/login")({
    validateSearch: (search: Record<string, unknown>) => ({
        redirect: (search.redirect as string) || undefined,
    }),
    beforeLoad: ({ context }) => {
        if (context.auth.user) {
            throw redirect({ to: "/" });
        }
    },
    component: LoginScreen,
});
