import type { LogEntry } from "@/hooks/use-log-stream";

function formatTimestamp(ts: string): string {
    try {
        const d = new Date(ts);
        if (Number.isNaN(d.getTime())) return ts;
        return d.toISOString().slice(11, 23);
    } catch {
        return ts;
    }
}

export function LogEntryRow({ entry }: { entry: LogEntry }) {
    return (
        <div className='flex items-start border-b border-border px-4 py-1.5'>
            <span className='w-[100px] shrink-0 font-mono text-[11px] text-muted-foreground'>
                {entry.timestamp ? formatTimestamp(entry.timestamp) : ""}
            </span>
            <span className='w-[70px] shrink-0'>
                <span
                    className={`inline-block rounded px-1.5 py-0.5 font-mono text-[9px] font-bold ${
                        entry.stream === "stderr" ? "bg-accent-red/15 text-accent-red" : "bg-accent-green/15 text-accent-green"
                    }`}
                >
                    {entry.stream === "stderr" ? "ERR" : "OUT"}
                </span>
            </span>
            <span className='min-w-0 flex-1 break-all font-mono text-[11px] text-foreground'>{entry.line}</span>
        </div>
    );
}
