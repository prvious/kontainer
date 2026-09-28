import { createFileRoute, redirect } from "@tanstack/react-router";
import { LoginScreen } from "@/components/login-screen";
import { AccentLine } from "@/components/accent-line";
import { Titlebar } from "@/components/shell/titlebar";

export const Route = createFileRoute("/login")({
    validateSearch: (search: Record<string, unknown>) => ({
        redirect: (search.redirect as string) || undefined,
    }),
    beforeLoad: ({ context, search }) => {
        if (context.auth.session) {
            throw redirect({ to: search.redirect || "/" });
        }
    },
    component: LoginLayout,
});

function LoginLayout() {
    return (
        <div className='flex h-screen flex-col bg-background'>
            <AccentLine />
            <Titlebar />
            <LoginScreen />
        </div>
    );
}
