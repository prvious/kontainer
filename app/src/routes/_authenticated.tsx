import { useEffect } from "react";
import { Outlet, createFileRoute, redirect, useRouter } from "@tanstack/react-router";
import { AccentLine } from "@/components/accent-line";
import { ClusterSwitcher } from "@/components/cluster-switcher";
import { SessionExpiredDialog } from "@/components/connect/session-expired-dialog";
import { SessionIssue } from "@/components/session-issue";
import { AppSidebar } from "@/components/shell/app-sidebar";
import { StatusBar } from "@/components/shell/status-bar";
import { Titlebar } from "@/components/shell/titlebar";
import { SidebarProvider } from "@/components/ui/sidebar";
import { ActionButton } from "@/components/kit/action-button";
import { useAuth } from "@/hooks/use-auth";
import { useConnectivity } from "@/hooks/use-connectivity";
import { useClusterSummary, useControllerStatus, useNodes } from "@/hooks/use-nodes";
import { useContainers } from "@/hooks/use-containers";
import { ConnectivityProvider } from "@/contexts/connectivity-context";
import { LogStreamsProvider } from "@/contexts/log-streams-context";
import { ExecSessionsProvider } from "@/contexts/exec-sessions-context";
import { useOpenShells } from "@/hooks/use-exec-sessions";
import { useOpenLogTabs } from "@/hooks/use-log-tabs";
import { OperationsProvider } from "@/contexts/operations-context";
import { clusterUsage, nodesDown } from "@/types/nodes";

function ShellError() {
    return (
        <div className='flex h-screen items-center justify-center bg-background text-foreground'>
            <div className='flex max-w-md flex-col items-center gap-4 text-center'>
                <p className='font-mono text-sm text-muted-foreground'>Something went wrong.</p>
                <ActionButton variant='outline' onClick={() => window.location.reload()}>
                    RELOAD
                </ActionButton>
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
    const { session, expired } = useAuth();
    return (
        <ConnectivityProvider enabled={Boolean(session) && !expired}>
            <LogStreamsProvider>
                <ExecSessionsProvider>
                    <AuthenticatedShell />
                </ExecSessionsProvider>
            </LogStreamsProvider>
        </ConnectivityProvider>
    );
}

function AuthenticatedShell() {
    const { session, expired, issue, retryCommit, retryRestore, logout, nextOrigin } = useAuth();
    const router = useRouter();
    const { probe, reachable } = useConnectivity();
    const logStreams = useOpenLogTabs();
    const terminals = useOpenShells();
    const nodes = useNodes();
    const summary = useClusterSummary();
    const controllerStatus = useControllerStatus();
    const containers = useContainers();

    // Signed out: leave for Connect, filled with the next agent when this was a switch. A switch
    // can't navigate there itself, because until this render the router still sees the session.
    useEffect(() => {
        if (session) return;
        void router
            .invalidate()
            .then(() => router.navigate({ to: "/login", search: { redirect: undefined, origin: nextOrigin ?? undefined } }))
            .catch(() => {});
    }, [session, router, nextOrigin]);

    async function retryLogout() {
        try {
            await logout();
        } catch {
            // The provider keeps the new closed error code visible.
        }
    }

    if (!session) return null;

    // Chrome stays put; only the main column scrolls.
    return (
        <OperationsProvider>
            <div className='flex h-screen flex-col overflow-hidden bg-background'>
                <AccentLine />
                <Titlebar center={<ClusterSwitcher health={probe} />} />
                {issue && issue !== "relogin_required" && (
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
                {/* While the session-expired dialog is up the page stays mounted but inert (02.S5). */}
                <SidebarProvider inert={expired || undefined} className='min-h-0 flex-1' style={{ "--sidebar-width": "220px" } as React.CSSProperties}>
                    <AppSidebar
                        badges={{ nodesDown: nodesDown(summary.data), containers: containers.data?.items.filter((row) => !row.stale).length }}
                        agent={controllerStatus.data && { version: controllerStatus.data.version, startedAt: controllerStatus.data.startedAt }}
                    />
                    <main className='min-w-0 flex-1 overflow-y-auto bg-background p-6'>
                        <Outlet />
                    </main>
                </SidebarProvider>
                <StatusBar
                    data={{
                        ...clusterUsage(summary.data, nodes.data?.items),
                        terminals,
                        logStreams,
                        connection: probe ? (reachable ? "connected" : "reconnecting") : undefined,
                    }}
                />
                <SessionExpiredDialog />
            </div>
        </OperationsProvider>
    );
}
