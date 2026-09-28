import type { ReactNode } from "react";
import { Container, Cpu, Layers, MemoryStick, ScrollText, Terminal, TriangleAlert, type LucideIcon } from "lucide-react";
import { StatusDot } from "@/components/kit/badges";
import { cn } from "@/lib/utils";

/** Everything the status bar shows; specs 05, 11, 15, and 16 fill the fields. Missing fields render "—". */
export interface StatusBarData {
    dockerVersion?: string;
    cpuPercent?: number;
    memoryUsedBytes?: number;
    memoryTotalBytes?: number;
    tasks?: number;
    alerts?: number;
    terminals?: number;
    logStreams?: number;
    /** Controller reachability from the health probe. */
    connection?: "connected" | "reconnecting";
}

const DASH = "—";
const gigabytes = (bytes: number) => (bytes / 1024 ** 3).toFixed(1).replace(/\.0$/, "");

function Segment({ icon: Icon, children, tone, edge }: { icon?: LucideIcon; children: ReactNode; tone?: "destructive"; edge: "left" | "right" | "none" }) {
    return (
        <span
            className={cn(
                "flex h-full items-center gap-1.25 px-2.5 font-mono text-3xs font-medium text-secondary-foreground",
                edge === "right" && "border-r border-border",
                edge === "left" && "border-l border-border",
                tone === "destructive" && "font-bold text-destructive",
            )}
        >
            {Icon && <Icon aria-hidden className={cn("size-2.75", tone === "destructive" ? "text-destructive" : "text-muted-foreground")} />}
            {children}
        </span>
    );
}

/**
 * Status bar `KYxT3`. The branch and Traefik segments from the design are not rendered.
 * Presentational only: the authenticated layout composes `StatusBarData` from the queries.
 */
export function StatusBar({ data }: { data: StatusBarData }) {
    const alerts = data.alerts;
    const memory =
        data.memoryUsedBytes !== undefined && data.memoryTotalBytes !== undefined
            ? `${gigabytes(data.memoryUsedBytes)}/${gigabytes(data.memoryTotalBytes)}G`
            : DASH;
    return (
        <footer className='flex h-7 shrink-0 items-center justify-between border-t border-border bg-statusbar px-3'>
            <div className='flex h-full items-center'>
                <Segment icon={Container} edge='right'>
                    Docker {data.dockerVersion ?? DASH}
                </Segment>
                <Segment icon={Cpu} edge='right'>
                    CPU {data.cpuPercent === undefined ? DASH : `${Math.round(data.cpuPercent)}%`}
                </Segment>
                <Segment icon={MemoryStick} edge='right'>
                    MEM {memory}
                </Segment>
                <Segment icon={Layers} edge='right'>
                    {data.tasks ?? DASH} TASKS
                </Segment>
                <Segment icon={TriangleAlert} edge='none' tone={alerts && alerts > 0 ? "destructive" : undefined}>
                    {alerts ?? DASH} {alerts === 1 ? "ALERT" : "ALERTS"}
                </Segment>
            </div>
            <div className='flex h-full items-center'>
                <Segment icon={Terminal} edge='left'>
                    {data.terminals ?? DASH} TERMINALS
                </Segment>
                <Segment icon={ScrollText} edge='left'>
                    {data.logStreams ?? DASH} LOG STREAMS
                </Segment>
                <Segment edge='left'>
                    {data.connection && <StatusDot tone={data.connection === "connected" ? "success" : "warning"} className='size-1.5' />}
                    {data.connection === "connected" ? "WS CONNECTED" : data.connection === "reconnecting" ? "RECONNECTING" : `WS ${DASH}`}
                </Segment>
            </div>
        </footer>
    );
}
