import { useEffect, useMemo, useRef } from "react";
import { useServiceLogs } from "@/hooks/use-service-logs";
import { LogEntryRow } from "@/components/logs/log-entry-row";

const DEFAULT_OPTIONS = {
    tail: "100",
    timestamps: true,
    stdout: true,
    stderr: true,
} as const;

interface LogPanelProps {
    serviceId: string;
    visible: boolean;
    logFilter: string;
}

export function LogPanel({ serviceId, visible, logFilter }: LogPanelProps) {
    const { logs, status } = useServiceLogs(serviceId, DEFAULT_OPTIONS);

    const scrollRef = useRef<HTMLDivElement>(null);

    // Auto-scroll to bottom when new logs arrive
    useEffect(() => {
        if (visible && scrollRef.current) {
            scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
        }
    }, [logs, visible]);

    const filteredLogs = useMemo(() => {
        if (!logFilter) return logs;
        const lower = logFilter.toLowerCase();
        return logs.filter((entry) => (entry.line ?? "").toLowerCase().includes(lower));
    }, [logs, logFilter]);

    const statusDotColor = {
        connected: "bg-success",
        connecting: "bg-warning",
        disconnected: "bg-muted-foreground",
        error: "bg-destructive",
    }[status];

    const emptyMessage = status === "connecting" ? "Connecting..." : logs.length === 0 && status === "connected" ? "Waiting for logs..." : "No logs";

    return (
        <div className='flex flex-col' style={{ display: visible ? "flex" : "none" }}>
            {/* Column header */}
            <div className='flex items-center bg-bg-elevated px-4 py-1.5'>
                <span className='w-[100px] shrink-0 font-mono text-[10px] font-bold tracking-wide text-muted-foreground uppercase'>Timestamp</span>
                <span className='w-[70px] shrink-0 font-mono text-[10px] font-bold tracking-wide text-muted-foreground uppercase'>Level</span>
                <span className='min-w-0 flex-1 font-mono text-[10px] font-bold tracking-wide text-muted-foreground uppercase'>Message</span>
            </div>

            {/* Scrollable log area */}
            <div ref={scrollRef} className='flex-1 overflow-y-auto'>
                {filteredLogs.length === 0 ? (
                    <div className='flex items-center justify-center px-4 py-8'>
                        <span className='font-mono text-[11px] text-muted-foreground'>{emptyMessage}</span>
                    </div>
                ) : (
                    filteredLogs.map((entry) => <LogEntryRow key={entry.id} entry={entry} />)
                )}
            </div>

            {/* Status bar */}
            <div className='flex items-center justify-between border-t border-border px-4 py-1.5'>
                <div className='flex items-center gap-1.5'>
                    <span className={`size-1.5 rounded-full ${statusDotColor}`} />
                    <span className='font-mono text-[10px] text-muted-foreground'>{status}</span>
                </div>
                <span className='font-mono text-[10px] text-muted-foreground'>{logs.length} lines</span>
            </div>
        </div>
    );
}
