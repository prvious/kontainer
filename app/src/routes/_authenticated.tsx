import { Outlet, createFileRoute, redirect, useRouter } from "@tanstack/react-router";
import { SidebarNav } from "@/components/sidebar-nav";
import { DashboardHeader } from "@/components/dashboard-header";
import { StatusBar } from "@/components/status-bar";
import { AccentLine } from "@/components/accent-line";
import { TerminalProvider } from "@/contexts/terminal-context";
import { CommandPaletteProvider } from "@/contexts/command-palette-context";
import { CommandPalette } from "@/components/command-palette";
import { useAuth } from "@/hooks/use-auth";
import { useAgentStatus } from "@/hooks/use-agent-status";

function ShellError({ error }: { error: Error }) {
    return (
        <div className='flex h-screen items-center justify-center bg-background text-foreground'>
            <div className='max-w-md text-center'>
                <p className='font-mono text-sm text-muted-foreground'>Something went wrong.</p>
                <p className='mt-2 font-mono text-xs text-muted-foreground/60'>{error.message}</p>
                <div className='mt-4 flex items-center justify-center gap-2'>
                    <button
                        className='rounded border border-border px-3 py-1.5 font-mono text-xs text-foreground transition-colors hover:bg-bg-elevated'
                        onClick={() => window.location.reload()}
                    >
                        Reload
                    </button>
                </div>
            </div>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated")({
    beforeLoad: ({ context, location }) => {
        if (!context.auth.user) {
            throw redirect({
                to: "/login",
                search: { redirect: location.href },
            });
        }
    },
    component: AuthenticatedLayout,
    errorComponent: ShellError,
});

function AuthenticatedLayout() {
    const { agentUrl, token, logout } = useAuth();
    const router = useRouter();

    useAgentStatus(agentUrl, token, () => {
        logout().then(() => router.navigate({ to: "/login", search: { redirect: undefined } }));
    });

    return (
        <TerminalProvider>
            <CommandPaletteProvider>
                <div className='flex h-screen flex-col overflow-hidden bg-background'>
                    <AccentLine />
                    <DashboardHeader />
                    <div className='flex flex-1 overflow-hidden'>
                        <SidebarNav />
                        <main className='flex-1 overflow-y-auto'>
                            <Outlet />
                        </main>
                    </div>
                    <StatusBar />
                </div>
                <CommandPalette />
            </CommandPaletteProvider>
        </TerminalProvider>
    );
}
