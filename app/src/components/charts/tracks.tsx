import { Bar, BarChart, ResponsiveContainer, XAxis, YAxis } from "recharts";
import type { Tone } from "@/components/kit/badges";

const toneFill: Record<Tone, string> = {
    success: "var(--success)",
    warning: "var(--warning)",
    destructive: "var(--destructive)",
    neutral: "var(--muted-foreground)",
    primary: "var(--primary)",
};

function HorizontalStack({
    segments,
    total,
    height,
    label,
}: {
    segments: { key: string; value: number; fill: string }[];
    total: number;
    height: number;
    label: string;
}) {
    const row: Record<string, number> = { remainder: Math.max(0, total - segments.reduce((sum, segment) => sum + segment.value, 0)) };
    for (const segment of segments) row[segment.key] = segment.value;
    return (
        <div role='img' aria-label={label} className='w-full overflow-hidden rounded-[2px]' style={{ height }}>
            <ResponsiveContainer width='100%' height={height} initialDimension={{ width: 200, height }}>
                <BarChart layout='vertical' data={[row]} margin={{ top: 0, right: 0, bottom: 0, left: 0 }} barCategoryGap={0}>
                    <XAxis type='number' hide domain={[0, Math.max(total, 1)]} />
                    <YAxis type='category' hide />
                    {segments.map((segment) => (
                        <Bar key={segment.key} dataKey={segment.key} stackId='track' fill={segment.fill} isAnimationActive={false} />
                    ))}
                    <Bar dataKey='remainder' stackId='track' fill='var(--secondary)' isAnimationActive={false} />
                </BarChart>
            </ResponsiveContainer>
        </div>
    );
}

/** Progress track from F2 `B64BE`: one value against a real maximum. `null` renders an empty track. */
export function Track({ value, max, height = 4, label }: { value: number | null; max: number; height?: number; label: string }) {
    return (
        <HorizontalStack
            label={label}
            height={height}
            total={max}
            segments={value === null ? [] : [{ key: "value", value: Math.min(value, max), fill: "var(--primary)" }]}
        />
    );
}

/** Segmented bar from F2 `B64BE`: parts of a bounded whole, coloured by state. */
export function StackedTrack({ segments, height = 4, label }: { segments: { key: string; value: number; tone: Tone }[]; height?: number; label: string }) {
    const total = segments.reduce((sum, segment) => sum + segment.value, 0);
    return (
        <HorizontalStack
            label={label}
            height={height}
            total={total}
            segments={segments.map((segment) => ({ key: segment.key, value: segment.value, fill: toneFill[segment.tone] }))}
        />
    );
}
