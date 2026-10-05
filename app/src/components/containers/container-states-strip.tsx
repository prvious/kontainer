import { Box } from "lucide-react";
import { StatusDot } from "@/components/kit/badges";
import { Ring } from "@/components/charts/ring";
import { SparkBars, type SparkBucket } from "@/components/charts/spark-bars";
import { useSamples } from "@/hooks/use-nodes";
import type { ContainerRow } from "@/hooks/use-containers";

const BUCKET_MS = 15 * 60 * 1000;

/**
 * Container States strip `DILCb`: legend counts, a RUNNING ring of running / total (the design's
 * "slot usage" is this real ratio), and 24 h of running containers from cluster samples.
 */
export function ContainerStatesStrip({ rows }: { rows: ContainerRow[] }) {
    const live = rows.filter((row) => !row.stale);
    const running = live.filter((row) => row.state === "running").length;
    const paused = live.filter((row) => row.state === "paused").length;
    const exited = live.filter((row) => row.state === "exited" || row.state === "dead").length;
    const samples = useSamples("24h", "15m");
    const buckets: SparkBucket[] = (samples.data?.points ?? []).map((point) => {
        const start = Date.parse(point.at);
        return { start, end: start + BUCKET_MS, value: point.containersRunning === null ? null : Math.round(point.containersRunning) };
    });

    return (
        <section aria-label='Container states' className='flex h-18 items-center justify-between gap-6 rounded-md bg-card px-4'>
            <div className='flex items-center gap-3'>
                <div className='flex size-7.5 items-center justify-center rounded-md bg-secondary'>
                    <Box aria-hidden className='size-3.75 text-secondary-foreground' />
                </div>
                <div className='flex flex-col gap-1.25'>
                    <h2 className='font-mono text-xs font-bold text-foreground'>CONTAINER STATES</h2>
                    <div className='flex items-center gap-3 font-mono text-3xs font-semibold text-secondary-foreground'>
                        <span className='flex items-center gap-1.25'>
                            <StatusDot tone='success' /> {running} RUNNING
                        </span>
                        <span className='flex items-center gap-1.25'>
                            <StatusDot tone='warning' /> {paused} PAUSED
                        </span>
                        <span className='flex items-center gap-1.25'>
                            <StatusDot tone='destructive' /> {exited} EXITED
                        </span>
                    </div>
                </div>
            </div>
            <div className='flex items-center gap-5.5'>
                <div className='flex items-center gap-2.5'>
                    <div className='flex flex-col gap-1.25'>
                        <span className='font-mono text-3xs font-bold text-muted-foreground'>RUNNING</span>
                        <span className='flex items-baseline gap-1.25'>
                            <span className='font-sans text-lg font-bold text-foreground'>{running}</span>
                            <span className='font-mono text-3xs font-medium text-secondary-foreground'>/ {live.length}</span>
                        </span>
                    </div>
                    <Ring
                        value={live.length === 0 ? null : running / live.length}
                        size={38}
                        thickness={4}
                        label={`${running} of ${live.length} containers running`}
                    />
                </div>
                <div aria-hidden className='h-6.5 w-px bg-border' />
                <div className='flex flex-col gap-1.5'>
                    <SparkBars buckets={buckets} width={118} height={30} label='Running containers, last 24 hours' />
                    <span className='font-mono text-3xs font-bold text-muted-foreground'>RUNNING · 24h</span>
                </div>
            </div>
        </section>
    );
}
