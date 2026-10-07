import { History, Play, RefreshCw, SearchX, X } from "lucide-react";
import { EmptyState } from "@/components/kit/region-states";
import { InlineAlert } from "@/components/kit/inline-alert";
import { LogViewer } from "@/components/logs/log-viewer";
import type { LogTab } from "@/contexts/log-streams-context-value";
import { useLogBuffer } from "@/hooks/use-log-tabs";
import { useNow } from "@/hooks/use-now";
import { clockTime } from "@/lib/time";
import { lineMatcher } from "@/types/logs";

/** The query-and-window sentence of the no-match state (08.S4). */
function windowText(tab: LogTab, lines: number, name: string, firstTs: number | null): string {
    const since = tab.window.since ?? firstTs ?? tab.window.openedAt;
    return `Searched the last ${lines.toLocaleString()} ${lines === 1 ? "line" : "lines"} of ${name} since ${clockTime(since)}. The filter runs on lines already received; new matches appear here live.`;
}

/**
 * One tab's body: the reconnecting alert (08.S3), the stopped banner with RESUME (08.S4), and the
 * viewer, or the empty states when nothing matches the filter or nothing has arrived yet.
 */
export function LogTabBody({
    tab,
    name,
    filter,
    wrap,
    onClearFilter,
    onLoadPreviousHour,
    onResume,
    onRetryNow,
}: {
    tab: LogTab;
    name: string;
    filter: string;
    wrap: boolean;
    onClearFilter: () => void;
    onLoadPreviousHour: () => void;
    onResume: () => void;
    onRetryNow: () => void;
}) {
    const snapshot = useLogBuffer(tab.buffer);
    const now = useNow();
    const matches = lineMatcher(filter);
    const filtering = filter.trim() !== "";
    const rows = filtering ? snapshot.rows.filter((row) => row.kind === "line" && matches(row.line.text)) : snapshot.rows;
    const lines = snapshot.rows.filter((row) => row.kind === "line").length;

    let body;
    if (filtering && rows.length === 0) {
        body = (
            <EmptyState
                icon={SearchX}
                title={`No lines match '${filter.trim()}'`}
                primaryAction={{ label: "CLEAR FILTER", icon: X, onClick: onClearFilter }}
                secondaryAction={{ label: "LOAD PREVIOUS HOUR", icon: History, onClick: onLoadPreviousHour }}
                className='flex-1 justify-center'
            >
                {windowText(tab, lines, name, snapshot.firstTs)}
            </EmptyState>
        );
    } else if (rows.length === 0) {
        body = (
            <EmptyState icon={History} title={snapshot.status === "connecting" ? "Opening the stream…" : "No lines yet"} className='flex-1 justify-center'>
                {`${name} hasn't written anything since ${clockTime(tab.window.since ?? tab.window.openedAt)}. New lines appear here live.`}
            </EmptyState>
        );
    } else {
        body = <LogViewer snapshot={snapshot} rows={rows} wrap={wrap} />;
    }

    const retryIn = snapshot.reconnect ? Math.max(0, Math.ceil((snapshot.reconnect.retryAt - now) / 1000)) : 0;
    return (
        <div className='flex min-h-0 flex-1 flex-col'>
            {snapshot.status === "reconnecting" && snapshot.reconnect && (
                <InlineAlert
                    tone='warning'
                    title={`Reconnecting to ${name}… attempt ${snapshot.reconnect.attempt} · next try in ${retryIn}s`}
                    action={{ label: "RETRY NOW", icon: RefreshCw, onClick: onRetryNow }}
                    className='rounded-none'
                >
                    {`The log stream dropped at ${clockTime(snapshot.droppedAt ?? now, true)}. Lines below are kept; new lines resume from where it stopped.`}
                </InlineAlert>
            )}
            {snapshot.ended && (
                <InlineAlert
                    tone='destructive'
                    title={snapshot.ended.reason === "service_removed" ? `${name} was removed — the stream stopped` : "The stream stopped"}
                    action={{ label: "RESUME", icon: Play, onClick: onResume }}
                    className='rounded-none'
                >
                    {snapshot.ended.reason === "service_removed"
                        ? "Lines received so far are kept. RESUME follows the service again once it is redeployed."
                        : `${snapshot.ended.message} (${snapshot.ended.code}). Lines received so far are kept.`}
                </InlineAlert>
            )}
            {body}
        </div>
    );
}
