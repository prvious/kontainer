import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/lib/utils";

export type Tone = "success" | "warning" | "destructive" | "neutral" | "primary";

const toneText: Record<Tone, string> = {
    success: "text-success",
    warning: "text-warning",
    destructive: "text-destructive",
    neutral: "text-muted-foreground",
    primary: "text-sidebar-accent-foreground",
};

const toneDot: Record<Tone, string> = {
    success: "bg-success",
    warning: "bg-warning",
    destructive: "bg-destructive",
    neutral: "bg-muted-foreground",
    primary: "bg-primary",
};

const toneSurface: Record<Tone, string> = {
    success: "border-success bg-success-soft",
    warning: "border-warning bg-warning-soft",
    destructive: "border-destructive bg-destructive-soft",
    neutral: "border-border bg-secondary",
    primary: "border-primary bg-primary-soft",
};

export function StatusDot({ tone, className, ...props }: { tone: Tone } & ComponentProps<"span">) {
    return <span aria-hidden data-tone={tone} className={cn("inline-block size-1.75 shrink-0 rounded-full", toneDot[tone], className)} {...props} />;
}

/** Badge/Status `Wbday`: dot and caps label on a soft tinted surface. */
export function StatusBadge({ tone, children, className }: { tone: Tone; children: ReactNode; className?: string }) {
    return (
        <span
            className={cn(
                "inline-flex items-center gap-1.5 rounded-sm border px-2.5 py-1 font-mono text-3xs font-bold uppercase",
                toneSurface[tone],
                toneText[tone],
                className,
            )}
        >
            <StatusDot tone={tone} className='size-1.5' />
            {children}
        </span>
    );
}

/** Badge `hcuLC`: neutral count or label chip. */
export function LabelBadge({ children, className }: { children: ReactNode; className?: string }) {
    return (
        <span
            className={cn(
                "inline-flex items-center rounded-sm border border-border bg-secondary px-1.5 py-0.5 font-mono text-3xs leading-none font-bold text-secondary-foreground",
                className,
            )}
        >
            {children}
        </span>
    );
}

/** StatusChip `4nr0v`: the table STATUS cell, e.g. UP / PAUSED / EXITED. */
export function StatusChip({ tone, children, className }: { tone: Tone; children: ReactNode; className?: string }) {
    return (
        <span className={cn("inline-flex items-center gap-1.5 font-mono text-2xs font-semibold uppercase", toneText[tone], className)}>
            <StatusDot tone={tone} />
            {children}
        </span>
    );
}
