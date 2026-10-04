import { ChevronRight } from "lucide-react";
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from "@/components/ui/context-menu";
import { LabelBadge, StatusDot, type Tone } from "@/components/kit/badges";
import { LabelTooltip } from "@/components/kit/tooltips";
import { useConnectionProbes, useForgetConnection } from "@/hooks/session/use-session-connections";
import { relativeTime } from "@/lib/time";
import { useNow } from "@/hooks/use-now";
import { connectionHost } from "@/components/connect/connect-rules";
import type { ProbeView, SavedConnection } from "@/types/session";

function probeTone(probe: ProbeView | undefined): Tone {
    if (!probe) return "neutral";
    return probe.status === "ready" ? "success" : "destructive";
}

/**
 * SAVED CONNECTIONS on the Connect card: each row is probed once (three at a time) for its dot;
 * clicking fills the URL, and the context menu or Delete on the focused row forgets it.
 */
export function SavedConnections({ connections, onChoose }: { connections: SavedConnection[]; onChoose: (connection: SavedConnection) => void }) {
    const probes = useConnectionProbes(connections.map((connection) => connection.origin));
    const forget = useForgetConnection();
    const now = useNow();
    if (connections.length === 0) return null;
    return (
        <section className='flex flex-col gap-3'>
            <div className='flex items-center justify-between'>
                <h2 className='font-mono text-3xs font-semibold tracking-[1px] text-muted-foreground uppercase'>Saved connections</h2>
                <LabelBadge>
                    {connections.length} {connections.length === 1 ? "AGENT" : "AGENTS"}
                </LabelBadge>
            </div>
            <ul className='overflow-hidden rounded-md border border-border'>
                {connections.map((connection) => {
                    const probe = probes.data?.[connection.origin];
                    const status = probe ? (probe.status === "ready" ? "Reachable" : "Offline") : "Not checked yet";
                    return (
                        <li key={connection.id} className='border-b border-border last:border-b-0'>
                            <ContextMenu>
                                <ContextMenuTrigger>
                                    <LabelTooltip label={`${status} · last used ${relativeTime(connection.lastUsedAtUnixMs, now)}`} shortcut='DEL to forget'>
                                        <button
                                            type='button'
                                            aria-keyshortcuts='Delete'
                                            onClick={() => onChoose(connection)}
                                            onKeyDown={(event) => {
                                                // The Mac's delete key reports Backspace.
                                                if (event.key !== "Delete" && event.key !== "Backspace") return;
                                                event.preventDefault();
                                                forget.mutate(connection.id);
                                            }}
                                            className='flex w-full items-center gap-3 px-3.5 py-3 text-left transition-colors hover:bg-secondary'
                                        >
                                            <StatusDot tone={probeTone(probe)} data-testid={`probe-${connection.id}`} />
                                            <span className='flex min-w-0 flex-1 flex-col gap-0.5'>
                                                <span className='truncate font-mono text-xs font-semibold text-foreground'>
                                                    {connection.label ?? connectionHost(connection.origin)}
                                                </span>
                                                <span className='truncate font-mono text-3xs text-muted-foreground'>{connectionHost(connection.origin)}</span>
                                            </span>
                                            <ChevronRight aria-hidden className='size-3.5 text-muted-foreground' />
                                        </button>
                                    </LabelTooltip>
                                </ContextMenuTrigger>
                                <ContextMenuContent>
                                    <ContextMenuItem variant='destructive' onClick={() => forget.mutate(connection.id)} className='font-mono text-2xs'>
                                        Forget this connection
                                    </ContextMenuItem>
                                </ContextMenuContent>
                            </ContextMenu>
                        </li>
                    );
                })}
            </ul>
        </section>
    );
}
