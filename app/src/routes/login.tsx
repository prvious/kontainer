import { createFileRoute, redirect } from "@tanstack/react-router";
import { LoginScreen } from "@/components/login-screen";
import { AccentLine } from "@/components/accent-line";
import { Logo } from "@/components/logo";

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
            <header data-tauri-drag-region className='flex h-[38px] shrink-0 items-center border-b border-border bg-titlebar-bg/90 px-4'>
                <div className='w-[52px] shrink-0' />
                <Logo />
            </header>
            <LoginScreen />
        </div>
    );
}
