import type { ReactNode } from "react";
import { Activity, Box, Clock, Cpu, MemoryStick, type LucideIcon } from "lucide-react";
import { StatusDot } from "@/components/kit/badges";
import { Ring } from "@/components/charts/ring";
import { StackedTrack } from "@/components/charts/tracks";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { bytes } from "@/components/containers/container-format";
import { cpuByNode, memoryFigures, networkRates, sampledNodeCount, type ContainerCounts } from "@/components/overview/overview-metrics";
import { clockTime } from "@/lib/time";
import type { ClusterSummary, NodeSummary } from "@/types/nodes";
import { cn } from "@/lib/utils";

const DASH = "—";
const gigabytes = (value: number) => (value / 1024 ** 3).toFixed(1).replace(/\.0$/, "");
const plural = (count: number, one: string) => `${count} ${one}${count === 1 ? "" : "s"}`;

/** Metric card from `ypkH5`: label and icon, a big figure, and a footer line; stale ones say as of when. */
function MetricCard({
    label,
    icon: Icon,
    staleAt,
    value,
    unit,
    aside,
    children,
}: {
    label: string;
    icon: LucideIcon;
    staleAt: number | null;
    value: string;
    unit?: string;
    aside?: ReactNode;
    children: ReactNode;
}) {
    return (
        <section aria-label={label} className='flex min-h-30 flex-col justify-between gap-3 rounded-md border border-border bg-card p-4'>
            <header className='flex items-center justify-between gap-2'>
                <h2 className='font-mono text-3xs font-medium tracking-[1.5px] text-muted-foreground'>{label}</h2>
                {staleAt === null ? (
                    <Icon aria-hidden className='size-3.5 text-muted-foreground' />
                ) : (
                    <span className='flex items-center gap-1 font-mono text-3xs text-warning'>
                        <Clock aria-hidden className='size-3' />
                        as of {clockTime(staleAt)}
                    </span>
                )}
            </header>
            <div className={cn("flex items-center justify-between gap-3", staleAt !== null && "opacity-50")}>
                <span className='flex items-baseline gap-1.5'>
                    <span className='font-sans text-3xl font-bold text-foreground'>{value}</span>
                    {unit && <span className='font-mono text-2xs font-medium text-secondary-foreground'>{unit}</span>}
                </span>
                {aside}
            </div>
            <div className={cn("flex flex-col gap-2 font-mono text-3xs font-medium text-secondary-foreground", staleAt !== null && "opacity-50")}>
                {children}
            </div>
        </section>
    );
}

/** The CPU ring; hovering or focusing it breaks CPU down per node, and a node without a live sample says so. */
function CpuRing({ percent, nodes }: { percent: number | null; nodes: NodeSummary[] }) {
    const rows = cpuByNode(nodes);
    return (
        <Tooltip>
            <TooltipTrigger
                render={
                    <span tabIndex={0} className='rounded-full outline-none focus-visible:ring-2 focus-visible:ring-ring/50'>
                        <Ring value={percent === null ? null : percent / 100} size={42} thickness={5} label='Aggregate CPU by node' />
                    </span>
                }
            />
            <TooltipContent side='bottom' className='flex w-60 flex-col items-stretch gap-1.5 px-2.75 py-2.25 font-mono'>
                <span className='text-2xs font-bold'>CPU by node · {rows.map((row) => row.cores).join(" + ")} cores</span>
                {rows.map((row) => (
                    <span key={row.id} className='flex items-center justify-between gap-3 text-3xs'>
                        <span className='truncate'>{row.name}</span>
                        <span className={cn(row.percent === null && "text-destructive")}>
                            {row.percent === null ? "unreachable" : `${Math.round(row.percent)}%`}
                        </span>
                    </span>
                ))}
            </TooltipContent>
        </Tooltip>
    );
}

/**
 * The Overview metrics row: containers by state, aggregate CPU with its hour peak, memory, and
 * network throughput. `staleAt` dims every figure and stamps the time they are from (03.S2).
 */
export function OverviewCards({
    counts,
    summary,
    nodes,
    peak,
    staleAt,
}: {
    counts: ContainerCounts;
    summary: ClusterSummary;
    nodes: NodeSummary[];
    peak: number | null;
    staleAt: number | null;
}) {
    const memory = memoryFigures(summary);
    const network = networkRates(nodes);
    const [rateValue, rateUnit] = network ? bytes(network.total).split(" ") : [DASH, "B"];
    return (
        <div className='grid grid-cols-2 gap-4 xl:grid-cols-4'>
            <MetricCard label='CONTAINERS' icon={Box} staleAt={staleAt} value={String(counts.running)} unit={`/ ${counts.total}`}>
                <StackedTrack
                    label={`${counts.running} running, ${counts.paused} paused, ${counts.exited} exited`}
                    segments={[
                        { key: "running", value: counts.running, tone: "success" },
                        { key: "paused", value: counts.paused, tone: "warning" },
                        { key: "exited", value: counts.exited, tone: "destructive" },
                    ]}
                />
                <span className='flex items-center gap-3'>
                    <span className='flex items-center gap-1.25'>
                        <StatusDot tone='success' /> {counts.running} RUNNING
                    </span>
                    <span className='flex items-center gap-1.25'>
                        <StatusDot tone='warning' /> {counts.paused} PAUSED
                    </span>
                    <span className='flex items-center gap-1.25'>
                        <StatusDot tone='destructive' /> {counts.exited} EXITED
                    </span>
                </span>
            </MetricCard>
            <MetricCard
                label='CPU USAGE'
                icon={Cpu}
                staleAt={staleAt}
                value={summary.cpuPercent === null ? DASH : String(Math.round(summary.cpuPercent))}
                unit='%'
                aside={<CpuRing percent={summary.cpuPercent} nodes={nodes} />}
            >
                <span>
                    across {plural(sampledNodeCount(nodes), "node")} · peak {peak === null ? DASH : `${Math.round(peak)}%`}
                </span>
            </MetricCard>
            <MetricCard
                label='MEMORY'
                icon={MemoryStick}
                staleAt={staleAt}
                value={memory ? gigabytes(memory.usedBytes) : DASH}
                unit={`GB / ${memory ? gigabytes(memory.totalBytes) : DASH} GB`}
            >
                <span>{memory ? `${Math.round(memory.percent)}% allocated · ${bytes(memory.freeBytes)} free` : "no node reporting"}</span>
            </MetricCard>
            <MetricCard label='NETWORK I/O' icon={Activity} staleAt={staleAt} value={rateValue} unit={`${rateUnit}/s`}>
                <span>{network ? `up ${bytes(network.tx)}/s · down ${bytes(network.rx)}/s` : "no node reporting"}</span>
            </MetricCard>
        </div>
    );
}
