import { Outlet, createFileRoute, redirect, useRouter } from "@tanstack/react-router";
import { SidebarNav } from "@/components/sidebar-nav";
import { DashboardHeader } from "@/components/dashboard-header";
import { StatusBar } from "@/components/status-bar";
import { TerminalProvider } from "@/contexts/terminal-context";
import { CommandPaletteProvider } from "@/contexts/command-palette-context";
import { CommandPalette } from "@/components/command-palette";
import { useAuth } from "@/hooks/use-auth";
import { useAgentStatus } from "@/hooks/use-agent-status";

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
                <div className='flex h-screen flex-col overflow-hidden'>
                    <div className='flex flex-1 overflow-hidden p-2 pl-0 pb-0'>
                        <SidebarNav />
                        <div className='flex flex-1 flex-col overflow-hidden'>
                            <DashboardHeader />
                            <div className='grain flex flex-1 flex-col overflow-hidden rounded-xl bg-background shadow-sm'>
                                <main className='flex-1 overflow-y-auto'>
                                    <Outlet />
                                </main>
                            </div>
                        </div>
                    </div>
                    <StatusBar />
                </div>
                <CommandPalette />
            </CommandPaletteProvider>
        </TerminalProvider>
    );
}
