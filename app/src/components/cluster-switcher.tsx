import { ChevronDown, LogOut, Plus, Server } from "lucide-react";
import { useNavigate } from "@tanstack/react-router";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { LabelBadge, StatusDot, type Tone } from "@/components/kit/badges";
import { TooltipRich } from "@/components/kit/tooltips";
import { notify } from "@/components/kit/notify";
import { connectionHost } from "@/components/connect/connect-rules";
import { useAuth } from "@/hooks/use-auth";
import { useConnectionProbes, useSessionConnections } from "@/hooks/session/use-session-connections";
import { relativeTime } from "@/lib/time";
import { useNow } from "@/hooks/use-now";
import type { ProbeView, SavedConnection } from "@/types/session";
import { cn } from "@/lib/utils";

type RowStatus = "active" | "healthy" | "offline" | "unknown";

const statusTone: Record<RowStatus, Tone> = { active: "primary", healthy: "success", offline: "neutral", unknown: "neutral" };
const statusBadge: Record<RowStatus, string> = {
    active: "border-primary bg-primary-soft text-sidebar-accent-foreground",
    healthy: "border-success bg-success-soft text-success",
    offline: "border-border bg-secondary text-muted-foreground",
    unknown: "border-border bg-secondary text-muted-foreground",
};

function rowStatus(connection: SavedConnection, activeOrigin: string | undefined, probe: ProbeView | undefined): RowStatus {
    if (connection.origin === activeOrigin) return "active";
    if (!probe) return "unknown";
    return probe.status === "ready" ? "healthy" : "offline";
}

function subtitle(connection: SavedConnection): string {
    const summary = connection.lastSummary;
    if (!summary) return connectionHost(connection.origin);
    return `${summary.nodes} ${summary.nodes === 1 ? "node" : "nodes"} · ${summary.services} ${summary.services === 1 ? "service" : "services"}`;
}

/**
 * The titlebar centre pill and the Cluster Switcher dropdown (`23tdn`, state 02.S6). Choosing
 * another agent signs out and opens Connect with that origin; offline agents are dimmed.
 */
export function ClusterSwitcher({ health }: { health?: ProbeView | null }) {
    const { session, logout } = useAuth();
    const navigate = useNavigate();
    const connections = useSessionConnections();
    const rows = connections.data ?? [];
    const probes = useConnectionProbes(rows.filter((connection) => connection.origin !== session?.origin).map((connection) => connection.origin));
    const label = session?.label ?? (session ? connectionHost(session.origin) : "No connection");
    const dot: Tone = health === undefined || health === null ? "neutral" : health.status === "ready" ? "success" : "destructive";
    const now = useNow();

    async function leaveFor(origin: string) {
        try {
            await logout();
        } catch (code) {
            notify.error("Couldn't sign out", { code: String(code), message: "You're still signed in to the current agent." });
            return;
        }
        void navigate({ to: "/login", search: { redirect: undefined, origin } });
    }

    return (
        <DropdownMenu>
            <DropdownMenuTrigger
                aria-label={`Switch cluster (current: ${label})`}
                className='flex items-center gap-2 rounded-sm px-2 py-1 font-mono text-2xs font-medium tracking-[0.5px] text-secondary-foreground transition-colors hover:text-foreground'
            >
                <StatusDot tone={dot} data-testid='controller-health-dot' />
                <span>{label}</span>
                <ChevronDown aria-hidden className='size-3 text-muted-foreground' />
            </DropdownMenuTrigger>
            <DropdownMenuContent align='center' className='w-80 p-0'>
                <div className='flex items-center justify-between border-b border-border px-4 py-3'>
                    <span className='flex items-center gap-2 font-mono text-3xs font-semibold tracking-[1px] text-muted-foreground uppercase'>
                        <Server aria-hidden className='size-3.5' /> Switch cluster
                    </span>
                    <LabelBadge>
                        {rows.length} {rows.length === 1 ? "agent" : "agents"}
                    </LabelBadge>
                </div>
                {rows.map((connection) => {
                    const status = rowStatus(connection, session?.origin, probes.data?.[connection.origin]);
                    const row = (
                        <DropdownMenuItem
                            key={connection.id}
                            disabled={status === "offline" || status === "active"}
                            onClick={() => void leaveFor(connection.origin)}
                            className={cn(
                                "flex items-center gap-3 rounded-none border-b border-border px-4 py-3",
                                status === "active" && "bg-primary-faint",
                                status === "offline" && "opacity-50",
                            )}
                        >
                            <StatusDot tone={statusTone[status]} />
                            <span className='flex min-w-0 flex-1 flex-col gap-0.5'>
                                <span className='truncate font-mono text-xs font-semibold text-foreground'>
                                    {connection.label ?? connectionHost(connection.origin)}
                                </span>
                                <span className='truncate font-mono text-3xs text-muted-foreground'>{subtitle(connection)}</span>
                            </span>
                            <span
                                data-testid={`switch-status-${connection.id}`}
                                className={cn("rounded-sm border px-1.5 py-0.5 font-mono text-3xs font-bold", statusBadge[status])}
                            >
                                {status === "unknown" ? "checking" : status}
                            </span>
                        </DropdownMenuItem>
                    );
                    return status === "offline" ? (
                        <TooltipRich
                            key={connection.id}
                            side='right'
                            title="Agent offline — can't switch"
                            reason={`Last used ${relativeTime(connection.lastUsedAtUnixMs, now)}. Start the agent on that host, or remove it from the saved connections.`}
                        >
                            <div>{row}</div>
                        </TooltipRich>
                    ) : (
                        row
                    );
                })}
                <DropdownMenuItem
                    onClick={() => void leaveFor("")}
                    className='justify-center gap-2 rounded-none py-3 font-mono text-2xs text-secondary-foreground'
                >
                    <Plus aria-hidden className='size-3.5' /> Add new agent
                </DropdownMenuItem>
                <DropdownMenuSeparator className='my-0' />
                <DropdownMenuItem
                    onClick={() => void logout().catch(() => {})}
                    className='justify-center gap-2 rounded-none py-3 font-mono text-2xs text-secondary-foreground'
                >
                    <LogOut aria-hidden className='size-3.5' /> Sign out
                </DropdownMenuItem>
            </DropdownMenuContent>
        </DropdownMenu>
    );
}
