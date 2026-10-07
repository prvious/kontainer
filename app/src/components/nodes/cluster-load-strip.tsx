import { Gauge } from "lucide-react";
import { StatusChip } from "@/components/kit/badges";
import { SparkBars, type SparkBucket } from "@/components/charts/spark-bars";
import { bytes } from "@/components/containers/container-format";
import { clockTime, shortDuration } from "@/lib/time";
import type { ClusterSummary, SamplePoint } from "@/types/nodes";

const BUCKET_MS = 10_000;
/**
 * The strip shows the last 2 minutes: twelve 10 s buckets of the 5 m cluster series. A bucket
 * spans two 5 s check-ins, so check-in jitter doesn't read as a node missing (spec 13, G29).
 */
export const TREND_BUCKETS = 12;

const plural = (count: number, one: string) => `${count} ${one}${count === 1 ? "" : "s"}`;
const gigabytes = (value: number) => (value / 1024 ** 3).toFixed(1).replace(/\.0$/, "");

function Figure({ label, value, unit }: { label: string; value: string; unit: string }) {
    return (
        <div className='flex flex-col gap-1.25'>
            <span className='font-mono text-3xs font-medium tracking-[1px] text-muted-foreground'>{label}</span>
            <span className='flex items-baseline gap-1.25'>
                <span className='font-sans text-lg font-bold text-foreground'>{value}</span>
                <span className='font-mono text-3xs font-medium text-secondary-foreground'>{unit}</span>
            </span>
        </div>
    );
}

/**
 * Cluster Load strip `xtTtS`: node, reachable, and container counts; quorum; aggregate CPU and
 * memory; and CPU over the last 2 minutes. Hovering a bar pins a crosshair and says how many nodes
 * reported in that bucket (04.S5).
 */
export function ClusterLoadStrip({ summary, points, now }: { summary: ClusterSummary | undefined; points: SamplePoint[]; now: number }) {
    // The series ends with the bucket still filling; it would read as nodes missing, so it's left out.
    const recent = points.slice(-TREND_BUCKETS - 1, -1);
    const buckets: SparkBucket[] = recent.map((point) => {
        const start = Date.parse(point.at);
        return { start, end: start + BUCKET_MS, value: point.cpuPercent === null ? null : Math.round(point.cpuPercent) };
    });
    const total = summary?.nodesTotal ?? 0;
    const reachable = summary ? summary.nodesTotal - summary.nodesDown : 0;
    const healthy = summary?.quorum === "healthy";
    // CPU is null exactly when no online node has a sample, so the sums mean nothing either.
    const sampled = summary !== undefined && summary.cpuPercent !== null;

    return (
        <section
            aria-label='Cluster load'
            className='flex min-h-18 flex-wrap items-center justify-between gap-6 rounded-md border border-border bg-card px-4 py-2'
        >
            <div className='flex items-center gap-3'>
                <div className='flex size-7.5 items-center justify-center rounded-md bg-secondary'>
                    <Gauge aria-hidden className='size-3.75 text-secondary-foreground' />
                </div>
                <div className='flex flex-col gap-1.25'>
                    <h2 className='font-mono text-xs font-bold text-foreground'>CLUSTER LOAD</h2>
                    <span className='font-mono text-3xs font-medium text-secondary-foreground'>
                        {summary ? `${plural(total, "node")} · ${reachable} reachable · ${plural(summary.containersRunning, "container")}` : "—"}
                    </span>
                </div>
            </div>
            <div className='flex items-center gap-5.5'>
                {summary && (
                    <StatusChip tone={healthy ? "success" : "warning"} className='text-3xs'>
                        {healthy ? "HEALTHY" : "DEGRADED"}
                    </StatusChip>
                )}
                <Figure label='AGGREGATE CPU' value={sampled ? String(Math.round(summary.cpuPercent ?? 0)) : "—"} unit='%' />
                <Figure
                    label='MEMORY'
                    value={sampled ? gigabytes(summary.memoryUsedBytes) : "—"}
                    unit={`GB / ${sampled ? gigabytes(summary.memoryTotalBytes) : "—"} GB`}
                />
                <div className='flex flex-col items-end gap-1.5'>
                    <SparkBars
                        buckets={buckets}
                        width={148}
                        height={30}
                        label='Aggregate CPU, last 2 minutes'
                        tooltipTime={(bucket) => `${clockTime(bucket.start, true)} · ${shortDuration((now - bucket.end) / 1000)} ago`}
                        tooltipRows={(_, index) => [
                            { label: "CPU", value: `${Math.round(recent[index].cpuPercent ?? 0)}%` },
                            { label: "Memory", value: bytes(recent[index].memoryUsedBytes), swatch: "neutral" },
                        ]}
                        tooltipDelta={(_, index) => ({
                            text: `${recent[index].nodes} of ${plural(total, "node")} reporting`,
                            tone: recent[index].nodes < total ? "warning" : "neutral",
                        })}
                    />
                    <span className='font-mono text-3xs font-medium tracking-[1px] text-muted-foreground'>LAST 2m</span>
                </div>
            </div>
        </section>
    );
}
