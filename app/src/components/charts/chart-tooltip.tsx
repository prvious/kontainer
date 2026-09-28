import { cn } from "@/lib/utils";

export type SeriesSwatch = "primary" | "success" | "warning" | "destructive" | "neutral";

const swatchClass: Record<SeriesSwatch, string> = {
    primary: "bg-primary",
    success: "bg-success",
    warning: "bg-warning",
    destructive: "bg-destructive",
    neutral: "bg-muted-foreground",
};

export interface ChartTooltipRow {
    label: string;
    value: string;
    swatch?: SeriesSwatch;
}

/** Chart Tooltip `V1sxxr`: bucket time range, one row per series, optional delta line. */
export function ChartTooltip({
    time,
    rows,
    delta,
    className,
}: {
    time: string;
    rows: ChartTooltipRow[];
    delta?: { text: string; tone: "success" | "warning" | "destructive" | "neutral" };
    className?: string;
}) {
    return (
        <div
            role='tooltip'
            className={cn(
                "pointer-events-none flex w-44 flex-col gap-1.5 rounded-md border border-border bg-popover px-2.5 py-2 font-mono shadow-lg",
                className,
            )}
        >
            <span className='text-3xs tracking-[0.5px] text-muted-foreground'>{time}</span>
            {rows.map((row) => (
                <div key={row.label} className='flex items-center gap-2 text-2xs'>
                    <span aria-hidden className={cn("size-2 shrink-0 rounded-[2px]", swatchClass[row.swatch ?? "primary"])} />
                    <span className='flex-1 truncate text-secondary-foreground'>{row.label}</span>
                    <span className='font-semibold text-foreground'>{row.value}</span>
                </div>
            ))}
            {delta && (
                <span
                    className={cn(
                        "text-3xs font-medium",
                        delta.tone === "success"
                            ? "text-success"
                            : delta.tone === "warning"
                              ? "text-warning"
                              : delta.tone === "destructive"
                                ? "text-destructive"
                                : "text-muted-foreground",
                    )}
                >
                    {delta.text}
                </span>
            )}
        </div>
    );
}
