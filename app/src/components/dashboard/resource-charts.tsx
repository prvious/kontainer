import { useState, useMemo } from "react";
import { Area, AreaChart, Bar, BarChart, CartesianGrid, XAxis, YAxis } from "recharts";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { ChartContainer, ChartTooltip, ChartTooltipContent } from "@/components/ui/chart";
import type { AgentMetricsSnapshot } from "@/types/metrics";

interface ResourceChartsProps {
    agents: AgentMetricsSnapshot[];
    history: AgentMetricsSnapshot[];
}

function generateFakeNetworkData() {
    const now = Date.now();
    const points = 12;
    const data = [];
    for (let i = points; i >= 0; i--) {
        const t = new Date(now - i * 3600000 * 2);
        data.push({
            time: t.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
            inbound: Math.round(50 + Math.random() * 150),
            outbound: Math.round(30 + Math.random() * 120),
        });
    }
    return data;
}

const cpuMemConfig = {
    cpu: { label: "CPU", color: "var(--color-chart-1)" },
    memory: { label: "Memory", color: "var(--color-chart-2)" },
};

const networkConfig = {
    inbound: { label: "Inbound", color: "var(--color-chart-1)" },
    outbound: { label: "Outbound", color: "var(--color-chart-2)" },
};

export function ResourceCharts({ agents, history }: ResourceChartsProps) {
    const hostnames = agents.map((a) => a.hostname);
    const [selectedNode, setSelectedNode] = useState<string>("all");

    const cpuMemData = useMemo(() => {
        if (history.length < 2) {
            return [];
        }
        const grouped: Record<string, { cpu: number[]; mem: number[] }> = {};
        for (const snap of history) {
            const key = new Date(snap.timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
            if (!grouped[key]) grouped[key] = { cpu: [], mem: [] };
            if (selectedNode === "all" || snap.hostname === selectedNode) {
                grouped[key].cpu.push(snap.node.cpuPercent);
                grouped[key].mem.push(snap.node.memoryPercent);
            }
        }
        return Object.entries(grouped).map(([time, vals]) => ({
            time,
            cpu: vals.cpu.reduce((a, b) => a + b, 0) / vals.cpu.length,
            memory: vals.mem.reduce((a, b) => a + b, 0) / vals.mem.length,
        }));
    }, [history, selectedNode]);

    const networkData = useMemo(() => generateFakeNetworkData(), []);

    return (
        <div className='grid gap-4 lg:grid-cols-2'>
            {/* CPU & Memory Chart */}
            <Card className='animate-fade-in-up' style={{ animationDelay: "200ms" }}>
                <CardHeader className='pb-2'>
                    <div className='flex items-start justify-between'>
                        <div>
                            <h3 className='text-sm font-medium'>CPU & Memory</h3>
                            <p className='text-xs text-muted-foreground'>
                                Usage over time
                                {selectedNode !== "all" && (
                                    <>
                                        {" "}
                                        — <span className='text-primary'>{selectedNode}</span>
                                    </>
                                )}
                            </p>
                        </div>
                        <div className='flex items-center gap-3'>
                            <div className='flex items-center gap-1.5'>
                                <span className='size-2 rounded-full bg-chart-1' />
                                <span className='text-[11px] text-muted-foreground'>CPU</span>
                            </div>
                            <div className='flex items-center gap-1.5'>
                                <span className='size-2 rounded-full bg-chart-2' />
                                <span className='text-[11px] text-muted-foreground'>Memory</span>
                            </div>
                        </div>
                    </div>
                    {/* Node selector */}
                    {hostnames.length > 0 && (
                        <div className='mt-2 flex flex-wrap gap-1'>
                            <button
                                onClick={() => setSelectedNode("all")}
                                className={`rounded-md px-2 py-1 text-[11px] font-medium transition-colors ${
                                    selectedNode === "all" ? 'bg-primary/10 text-primary' : 'text-muted-foreground hover:bg-accent'
                                }`}
                            >
                                All Nodes
                            </button>
                            {hostnames.map((h) => (
                                <button
                                    key={h}
                                    onClick={() => setSelectedNode(h)}
                                    className={`rounded-md px-2 py-1 font-mono text-[11px] transition-colors ${
                                        selectedNode === h ? 'bg-primary/10 text-primary' : 'text-muted-foreground hover:bg-accent'
                                    }`}
                                >
                                    {h}
                                </button>
                            ))}
                        </div>
                    )}
                </CardHeader>
                <CardContent className='pb-4'>
                    {cpuMemData.length === 0 ? (
                        <div className='flex h-[200px] items-center justify-center'>
                            <p className='text-sm text-muted-foreground'>Collecting metrics data...</p>
                        </div>
                    ) : (
                        <ChartContainer config={cpuMemConfig} className='h-[200px] w-full'>
                            <AreaChart data={cpuMemData} margin={{ top: 5, right: 5, bottom: 0, left: -20 }}>
                                <defs>
                                    <linearGradient id='cpuGradient' x1='0' y1='0' x2='0' y2='1'>
                                        <stop offset='5%' stopColor='var(--color-chart-1)' stopOpacity={0.3} />
                                        <stop offset='95%' stopColor='var(--color-chart-1)' stopOpacity={0} />
                                    </linearGradient>
                                    <linearGradient id='memGradient' x1='0' y1='0' x2='0' y2='1'>
                                        <stop offset='5%' stopColor='var(--color-chart-2)' stopOpacity={0.3} />
                                        <stop offset='95%' stopColor='var(--color-chart-2)' stopOpacity={0} />
                                    </linearGradient>
                                </defs>
                                <CartesianGrid vertical={false} strokeDasharray='3 3' className='stroke-border' />
                                <XAxis dataKey='time' tickLine={false} axisLine={false} tick={{ fontSize: 10 }} className='fill-muted-foreground' />
                                <YAxis tickLine={false} axisLine={false} tick={{ fontSize: 10 }} domain={[0, 100]} unit='%' className='fill-muted-foreground' />
                                <ChartTooltip content={<ChartTooltipContent />} />
                                <Area type='monotone' dataKey='cpu' stroke='var(--color-chart-1)' fill='url(#cpuGradient)' strokeWidth={2} />
                                <Area type='monotone' dataKey='memory' stroke='var(--color-chart-2)' fill='url(#memGradient)' strokeWidth={2} />
                            </AreaChart>
                        </ChartContainer>
                    )}
                </CardContent>
            </Card>

            {/* Network I/O Chart */}
            <Card className='animate-fade-in-up' style={{ animationDelay: "250ms" }}>
                <CardHeader className='pb-2'>
                    <div className='flex items-start justify-between'>
                        <div>
                            <h3 className='text-sm font-medium'>Network I/O</h3>
                            <p className='text-xs text-muted-foreground'>Traffic in MB/s (placeholder data)</p>
                        </div>
                        <div className='flex items-center gap-3'>
                            <div className='flex items-center gap-1.5'>
                                <span className='size-2 rounded-full bg-chart-1' />
                                <span className='text-[11px] text-muted-foreground'>Inbound</span>
                            </div>
                            <div className='flex items-center gap-1.5'>
                                <span className='size-2 rounded-full bg-chart-2' />
                                <span className='text-[11px] text-muted-foreground'>Outbound</span>
                            </div>
                        </div>
                    </div>
                </CardHeader>
                <CardContent className='pb-4'>
                    <ChartContainer config={networkConfig} className='h-[200px] w-full'>
                        <BarChart data={networkData} margin={{ top: 5, right: 5, bottom: 0, left: -20 }}>
                            <CartesianGrid vertical={false} strokeDasharray='3 3' className='stroke-border' />
                            <XAxis dataKey='time' tickLine={false} axisLine={false} tick={{ fontSize: 10 }} className='fill-muted-foreground' />
                            <YAxis tickLine={false} axisLine={false} tick={{ fontSize: 10 }} className='fill-muted-foreground' />
                            <ChartTooltip content={<ChartTooltipContent />} />
                            <Bar dataKey='inbound' fill='var(--color-chart-1)' radius={[3, 3, 0, 0]} />
                            <Bar dataKey='outbound' fill='var(--color-chart-2)' radius={[3, 3, 0, 0]} />
                        </BarChart>
                    </ChartContainer>
                </CardContent>
            </Card>
        </div>
    );
}
