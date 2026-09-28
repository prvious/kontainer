import { useEffect } from "react";
import { Link, Outlet, createFileRoute, redirect, useLocation, useRouter } from "@tanstack/react-router";
import { AccentLine } from "@/components/accent-line";
import { ClusterSwitcher } from "@/components/cluster-switcher";
import { Logo } from "@/components/logo";
import { SessionIssue } from "@/components/session-issue";
import { useAuth } from "@/hooks/use-auth";

function ShellError() {
    return (
        <div className='flex h-screen items-center justify-center bg-background text-foreground'>
            <div className='max-w-md text-center'>
                <p className='font-mono text-sm text-muted-foreground'>Something went wrong.</p>
                <button
                    className='mt-4 rounded border border-border px-3 py-1.5 font-mono text-xs text-foreground transition-colors hover:bg-bg-elevated'
                    onClick={() => window.location.reload()}
                >
                    Reload
                </button>
            </div>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated")({
    beforeLoad: ({ context, location }) => {
        if (!context.auth.session) {
            throw redirect({ to: "/login", search: { redirect: location.href } });
        }
    },
    component: AuthenticatedLayout,
    errorComponent: ShellError,
});

function AuthenticatedLayout() {
    const { session, issue, retryCommit, retryRestore, logout } = useAuth();
    const router = useRouter();
    const pathname = useLocation({ select: (location) => location.pathname });
    const settingsOpen = pathname === "/settings" || pathname === "/settings/";
    const containersOpen = pathname === "/tasks" || pathname === "/tasks/";

    useEffect(() => {
        if (session) return;
        void router
            .invalidate()
            .then(() => router.navigate({ to: "/login", search: { redirect: undefined } }))
            .catch(() => {});
    }, [session, router]);

    async function retryLogout() {
        try {
            await logout();
        } catch {
            // The provider keeps the new closed error code visible.
        }
    }

    if (!session) return null;

    return (
        <div className='flex h-screen flex-col bg-background'>
            <AccentLine />
            <header data-tauri-drag-region className='flex h-[38px] shrink-0 items-center justify-between border-b border-border bg-titlebar-bg/90 px-4'>
                <Link to='/' aria-label='Home'>
                    <Logo />
                </Link>
                <div className='flex items-center gap-5'>
                    <Link to='/tasks' className='font-mono text-xs text-muted-foreground hover:text-foreground'>
                        Containers
                    </Link>
                    <Link to='/settings' className='font-mono text-xs text-muted-foreground hover:text-foreground'>
                        Settings
                    </Link>
                    <ClusterSwitcher variant='titlebar' />
                </div>
            </header>
            {issue && (
                <SessionIssue
                    code={issue}
                    onRetryCommit={() => {
                        void retryCommit().catch(() => {});
                    }}
                    onRetryRestore={() => {
                        void retryRestore().catch(() => {});
                    }}
                    onLogout={() => {
                        void retryLogout();
                    }}
                />
            )}
            {settingsOpen || containersOpen ? (
                <main className='flex-1 overflow-y-auto'>
                    <Outlet />
                </main>
            ) : (
                <main className='flex flex-1 items-center justify-center p-6'>
                    <div className='w-full max-w-md space-y-4 border border-border bg-card p-6'>
                        <h1 className='text-xl font-semibold'>Connected</h1>
                        <p className='text-sm text-muted-foreground'>
                            Signed in as {session.githubLogin} to {session.origin}
                        </p>
                        <p className='text-sm text-muted-foreground'>Browse the configured node’s containers.</p>
                        <Link to='/tasks' className='inline-block text-sm text-primary underline-offset-4 hover:underline'>
                            Open Containers
                        </Link>
                    </div>
                </main>
            )}
        </div>
    );
}
