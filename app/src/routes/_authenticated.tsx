import { Component, type ErrorInfo, type ReactNode } from "react";
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

class ShellErrorBoundary extends Component<{ children: ReactNode }, { hasError: boolean }> {
    constructor(props: { children: ReactNode }) {
        super(props);
        this.state = { hasError: false };
    }

    static getDerivedStateFromError(): { hasError: boolean } {
        return { hasError: true };
    }

    componentDidCatch(error: Error, info: ErrorInfo) {
        console.error("[shell] render error:", error, info.componentStack);
    }

    render() {
        if (this.state.hasError) {
            return (
                <div className='flex h-screen items-center justify-center bg-background text-foreground'>
                    <div className='text-center'>
                        <p className='font-mono text-sm text-muted-foreground'>Something went wrong.</p>
                        <button
                            className='mt-3 rounded border border-border px-3 py-1.5 font-mono text-xs text-foreground transition-colors hover:bg-bg-elevated'
                            onClick={() => {
                                this.setState({ hasError: false });
                                window.location.reload();
                            }}
                        >
                            Reload
                        </button>
                    </div>
                </div>
            );
        }
        return this.props.children;
    }
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
});

function AuthenticatedLayout() {
    const { agentUrl, token, logout } = useAuth();
    const router = useRouter();

    useAgentStatus(agentUrl, token, () => {
        logout().then(() => router.navigate({ to: "/login", search: { redirect: undefined } }));
    });

    return (
        <ShellErrorBoundary>
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
        </ShellErrorBoundary>
    );
}
