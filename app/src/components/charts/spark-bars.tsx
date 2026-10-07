import { useState, type ComponentProps, type KeyboardEvent, type ReactNode } from "react";
import { Bar, BarChart, Cell, YAxis } from "recharts";
import { ChartTooltip, type ChartTooltipRow } from "@/components/charts/chart-tooltip";

export interface SparkBucket {
    /** Bucket start and end, epoch milliseconds. */
    start: number;
    end: number;
    /** `null` is a gap: no bar, and the tooltip says "no data" instead of 0. */
    value: number | null;
}

function clock(ms: number) {
    return new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
}

/**
 * Bar sparkline from F2 `B64BE`. One accent series; hovering (or ←/→ with focus) snaps a crosshair
 * to the nearest bucket, keeps that bar in the accent colour, and drops the rest to `--primary-deep`
 * (state `UWkEX`).
 */
export function SparkBars({
    buckets,
    width = 120,
    height = 28,
    label,
    tooltipRows,
    tooltipTime,
    tooltipDelta,
}: {
    buckets: SparkBucket[];
    width?: number;
    height?: number;
    /** Accessible name, e.g. "Running containers, last 24 hours". */
    label: string;
    /** Rows for the hovered bucket; defaults to one row with the raw value. */
    tooltipRows?: (bucket: SparkBucket, index: number) => ChartTooltipRow[];
    /** The tooltip's time line; defaults to the bucket's `HH:MM – HH:MM`. */
    tooltipTime?: (bucket: SparkBucket) => string;
    /** A footer line under the rows, e.g. how many nodes contributed. */
    tooltipDelta?: (bucket: SparkBucket, index: number) => ComponentProps<typeof ChartTooltip>["delta"];
}) {
    const [active, setActive] = useState<number | null>(null);
    const data = buckets.map((bucket, index) => ({ index, value: bucket.value }));
    const hovered = active === null ? null : buckets[active];
    const barWidth = buckets.length > 0 ? width / buckets.length : width;

    function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
        if (buckets.length === 0) return;
        if (event.key === "ArrowLeft") {
            event.preventDefault();
            setActive((current) => Math.max(0, (current ?? buckets.length) - 1));
        } else if (event.key === "ArrowRight") {
            event.preventDefault();
            setActive((current) => Math.min(buckets.length - 1, (current ?? -1) + 1));
        } else if (event.key === "Escape") {
            setActive(null);
        }
    }

    let tooltip: ReactNode = null;
    if (hovered && active !== null) {
        const rows =
            hovered.value === null
                ? [{ label: "no data", value: "—", swatch: "neutral" as const }]
                : (tooltipRows?.(hovered, active) ?? [{ label: label, value: String(hovered.value) }]);
        const left = Math.min(Math.max(0, active * barWidth + barWidth / 2 - 88), Math.max(0, width - 176));
        tooltip = (
            <ChartTooltip
                time={tooltipTime?.(hovered) ?? `${clock(hovered.start)} – ${clock(hovered.end)}`}
                rows={rows}
                delta={tooltipDelta?.(hovered, active)}
            />
        );
        tooltip = (
            <div className='absolute bottom-full z-10 mb-2' style={{ left }}>
                {tooltip}
            </div>
        );
    }

    return (
        <div
            role='img'
            aria-label={label}
            tabIndex={0}
            onKeyDown={onKeyDown}
            onBlur={() => setActive(null)}
            className='relative outline-none focus-visible:ring-2 focus-visible:ring-ring/50'
            style={{ width, height }}
        >
            <BarChart
                width={width}
                height={height}
                data={data}
                barCategoryGap={1}
                margin={{ top: 0, right: 0, bottom: 0, left: 0 }}
                onMouseMove={(state) => setActive(typeof state?.activeTooltipIndex === "number" ? state.activeTooltipIndex : null)}
                onMouseLeave={() => setActive(null)}
            >
                <YAxis hide domain={[0, "dataMax"]} />
                {/* A zero still shows a 1 px bar; a gap shows none. */}
                <Bar dataKey='value' isAnimationActive={false} minPointSize={(value) => (value === null ? 0 : 1)}>
                    {data.map((entry) => (
                        <Cell
                            key={entry.index}
                            data-active={entry.index === active || undefined}
                            fill={active === null || entry.index === active ? "var(--primary)" : "var(--primary-deep)"}
                        />
                    ))}
                </Bar>
            </BarChart>
            {active !== null && (
                <div aria-hidden className='pointer-events-none absolute inset-y-0 w-px bg-foreground/30' style={{ left: active * barWidth + barWidth / 2 }} />
            )}
            {tooltip}
        </div>
    );
}
