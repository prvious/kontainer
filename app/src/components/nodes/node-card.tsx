import { Progress as ProgressPrimitive } from "@base-ui/react/progress";
import { Server, TriangleAlert, WifiOff } from "lucide-react";
import { LabelBadge, StatusDot, type Tone } from "@/components/kit/badges";
import { ProgressIndicator, ProgressTrack } from "@/components/ui/progress";
import { bytes } from "@/components/containers/container-format";
import { healthNote, roleLabel } from "@/components/nodes/node-format";
import type { NodeHealth, NodeSummary } from "@/types/nodes";
import { cn } from "@/lib/utils";

const HEALTH_TONE: Record<NodeHealth, Tone> = { active: "success", degraded: "warning", down: "destructive" };
const HEALTH_TEXT: Record<NodeHealth, string> = { active: "text-success", degraded: "text-warning", down: "text-destructive" };

function InfoRow({ label, value, muted }: { label: string; value: string; muted: boolean }) {
    return (
        <div className='flex items-center justify-between gap-3 font-mono text-3xs'>
            <span className='shrink-0 text-muted-foreground'>{label}</span>
            <span className={cn("truncate", muted ? "text-muted-foreground" : "text-secondary-foreground")}>{value}</span>
        </div>
    );
}

function Track({
    label,
    value,
    fraction,
    indicatorClass,
    muted,
}: {
    label: string;
    value: string;
    fraction: number | null;
    indicatorClass: string;
    muted: boolean;
}) {
    return (
        <ProgressPrimitive.Root
            value={fraction === null ? null : Math.min(100, Math.max(0, fraction * 100))}
            aria-label={label}
            className='flex flex-col gap-1'
        >
            <div className='flex items-center justify-between gap-3 font-mono text-3xs'>
                <span className='text-muted-foreground uppercase'>{label}</span>
                <span className={muted ? "text-muted-foreground" : "text-secondary-foreground"}>{value}</span>
            </div>
            <ProgressTrack className='h-1 rounded-[2px] bg-secondary'>
                <ProgressIndicator className={indicatorClass} />
            </ProgressTrack>
        </ProgressPrimitive.Root>
    );
}

/**
 * Node card from `874nN`: name, role, and health; address, platform, Docker, and containers;
 * CPU and memory tracks. Degraded cards dim their figures under an amber note (04.S5); down cards
 * drop live figures to "—" over a destructive banner.
 */
export function NodeCard({ node, now }: { node: NodeSummary; now: number }) {
    const down = node.health === "down";
    const sample = down ? null : node.latestSample;
    const note = healthNote(node, now);
    const platform = [node.os, node.arch].filter(Boolean).join("/");
    const cores = `${node.cpuCores} ${node.cpuCores === 1 ? "core" : "cores"}`;
    const memoryTotal = node.memoryTotalBytes > 0 ? bytes(node.memoryTotalBytes) : "—";

    return (
        <article aria-label={node.hostname || node.id} data-health={node.health} className='flex flex-col gap-4 rounded-md bg-card p-5'>
            <header className='flex items-center justify-between gap-3'>
                <div className='flex min-w-0 items-center gap-2'>
                    <Server aria-hidden className='size-4 shrink-0 text-muted-foreground' />
                    <h2 className='truncate font-mono text-sm font-medium text-foreground'>{node.hostname || node.id}</h2>
                </div>
                <div className='flex shrink-0 items-center gap-2'>
                    {node.availability && node.availability !== "active" && <LabelBadge className='text-warning'>{node.availability.toUpperCase()}</LabelBadge>}
                    <LabelBadge>{roleLabel(node)}</LabelBadge>
                    <span className={cn("flex items-center gap-1 font-mono text-3xs font-medium", HEALTH_TEXT[node.health])}>
                        <StatusDot tone={HEALTH_TONE[node.health]} className='size-1.5' />
                        {node.health}
                    </span>
                </div>
            </header>
            <div className={cn("flex flex-col gap-4", node.health === "degraded" && "opacity-60")}>
                <div className='flex flex-col gap-2.5'>
                    <InfoRow muted={down} label='IP Address' value={node.address ?? "—"} />
                    <InfoRow muted={down} label='OS / Arch' value={platform || "—"} />
                    <InfoRow muted={down} label='Docker' value={node.dockerVersion || "—"} />
                    <InfoRow muted={down} label='Containers' value={sample ? `${sample.containersRunning} running` : "—"} />
                </div>
                <div className='flex flex-col gap-2'>
                    <Track
                        muted={down}
                        label='CPU'
                        value={`${sample ? `${Math.round(sample.cpuPercent)}%` : "—"} · ${cores}`}
                        fraction={sample ? sample.cpuPercent / 100 : null}
                        indicatorClass='bg-primary'
                    />
                    <Track
                        muted={down}
                        label='Memory'
                        value={`${sample ? bytes(sample.memoryUsedBytes) : "—"} / ${memoryTotal}`}
                        fraction={sample && node.memoryTotalBytes > 0 ? sample.memoryUsedBytes / node.memoryTotalBytes : null}
                        indicatorClass='bg-muted-foreground'
                    />
                </div>
            </div>
            {note && (
                <p
                    role={down ? "alert" : "status"}
                    className={cn(
                        "flex items-center gap-2 rounded-sm px-3 py-2 font-mono text-3xs",
                        down ? "bg-destructive-faint text-destructive" : "bg-warning-soft text-warning",
                    )}
                >
                    {down ? <TriangleAlert aria-hidden className='size-3.5 shrink-0' /> : <WifiOff aria-hidden className='size-3.5 shrink-0' />}
                    {note}
                </p>
            )}
        </article>
    );
}
