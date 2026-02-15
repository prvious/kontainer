import { Outlet, createFileRoute, redirect } from "@tanstack/react-router";
import { SidebarNav } from "@/components/sidebar-nav";
import { DashboardHeader } from "@/components/dashboard-header";
import { TerminalProvider } from "@/contexts/terminal-context";
import { CommandPaletteProvider } from "@/contexts/command-palette-context";
import { CommandPalette } from "@/components/command-palette";

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
    return (
        <TerminalProvider>
            <CommandPaletteProvider>
                <div className='flex h-screen overflow-hidden p-2 pl-0'>
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
                <CommandPalette />
            </CommandPaletteProvider>
        </TerminalProvider>
    );
}
