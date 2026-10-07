import { CircleCheck, CircleHelp, CircleSlash, CircleX, Clock, LoaderCircle, type LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";

export type OperationPhase = "queued" | "running" | "succeeded" | "failed" | "cancelled" | "uncertain";

const phaseIcon: Record<OperationPhase, { icon: LucideIcon; className: string }> = {
    queued: { icon: Clock, className: "text-muted-foreground" },
    running: { icon: LoaderCircle, className: "animate-spin text-primary" },
    succeeded: { icon: CircleCheck, className: "text-success" },
    failed: { icon: CircleX, className: "text-destructive" },
    cancelled: { icon: CircleSlash, className: "text-muted-foreground" },
    uncertain: { icon: CircleHelp, className: "text-warning" },
};

const trackFill: Record<OperationPhase, string> = {
    queued: "bg-muted-foreground",
    running: "bg-primary",
    succeeded: "bg-success",
    failed: "bg-destructive",
    cancelled: "bg-muted-foreground",
    uncertain: "bg-warning",
};

/**
 * Operation Status `nBaer` (+ Queued / Succeeded / Failed): title, a status line such as
 * `RUNNING · 4 of 5 tasks running · 12s`, a track, and one text action (CANCEL, DISMISS, DETAILS).
 */
export function OperationStatus({
    phase,
    title,
    meta,
    progress,
    action,
    className,
}: {
    phase: OperationPhase;
    title: string;
    meta: string;
    /** 0–1; `null` renders an empty track. */
    progress?: number | null;
    action?: { label: string; onClick: () => void };
    className?: string;
}) {
    const { icon: Icon, className: iconClass } = phaseIcon[phase];
    const fill = progress === null || progress === undefined ? 0 : Math.min(1, Math.max(0, progress));
    return (
        <div role='status' data-phase={phase} className={cn("flex items-center gap-3 rounded-md border border-border bg-card px-3.5 py-3", className)}>
            <Icon aria-hidden className={cn("size-3.5 shrink-0", iconClass)} />
            <div className='flex min-w-0 flex-1 flex-col gap-1'>
                <span className='truncate font-mono text-xs font-medium text-foreground'>{title}</span>
                <span className='truncate font-mono text-3xs tracking-[0.5px] text-muted-foreground'>{meta}</span>
            </div>
            <div className='h-1 w-24 shrink-0 overflow-hidden rounded-[2px] bg-secondary' aria-hidden>
                <div className={cn("h-full rounded-[2px] transition-[width]", trackFill[phase])} style={{ width: `${fill * 100}%` }} />
            </div>
            {action && (
                <button
                    type='button'
                    onClick={action.onClick}
                    className='shrink-0 font-mono text-3xs font-semibold tracking-[0.5px] text-secondary-foreground uppercase hover:text-foreground'
                >
                    {action.label}
                </button>
            )}
        </div>
    );
}
