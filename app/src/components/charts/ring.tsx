import { Cell, Pie, PieChart } from "recharts";

/** Ring gauge from F2 `B64BE`: one saturation figure. `null` renders an empty ring (no data), never 0. */
export function Ring({ value, size = 44, thickness = 5, label }: { value: number | null; size?: number; thickness?: number; label: string }) {
    const fraction = value === null ? 0 : Math.min(1, Math.max(0, value));
    const data = [{ value: fraction }, { value: 1 - fraction }];
    return (
        <div role='img' aria-label={label} style={{ width: size, height: size }}>
            <PieChart width={size} height={size}>
                <Pie
                    data={data}
                    dataKey='value'
                    startAngle={90}
                    endAngle={-270}
                    innerRadius={size / 2 - thickness}
                    outerRadius={size / 2}
                    stroke='none'
                    isAnimationActive={false}
                >
                    <Cell fill={value === null ? "var(--secondary)" : "var(--primary)"} />
                    <Cell fill='var(--secondary)' />
                </Pie>
            </PieChart>
        </div>
    );
}
