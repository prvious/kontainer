import { useState, useRef, useEffect } from "react";
import { ChevronDown, Cpu, ListChecks, MemoryStick, Server, TrendingDown, TrendingUp } from "lucide-react";
import type { NodeItem } from "@/types/nodes";
import type { AgentMetricsSnapshot } from "@/types/metrics";
import type { TaskStats } from "@/types/task-stats";

interface StatsCardsProps {
    nodes: NodeItem[];
    agents: AgentMetricsSnapshot[];
    taskStats: TaskStats;
}

function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

interface NodeHealthItem {
    name: string;
    role: string;
    status: string;
    availability: string;
    leader: boolean;
}

interface StatCardData {
    label: string;
    value: string;
    icon: React.ComponentType<{ className?: string }>;
    trend: "up" | "down" | "neutral";
    trendValue: string;
    detail: string;
    nodes?: { name: string; value: number; max: number }[];
    healthNodes?: NodeHealthItem[];
}

// Responsive border classes per position in a 1→2→4 column grid
const statBorders = [
    "border-b sm:border-r lg:border-b-0", // item 0
    "border-b lg:border-b-0 lg:border-r", // item 1
    "border-b sm:border-b-0 sm:border-r", // item 2
    "", // item 3 (last, no borders)
];

function StatCard({ stat, delay, borderClass }: { stat: StatCardData; delay: number; borderClass: string }) {
    return (
        <div className={`animate-fade-in-up border-border p-4 ${borderClass}`} style={{ animationDelay: `${delay}ms` }}>
            <div className='flex items-start justify-between'>
                <div>
                    <p className='text-[11px] font-medium tracking-wider text-muted-foreground uppercase'>{stat.label}</p>
                    <p className='mt-1 font-mono text-2xl font-semibold tracking-tight'>{stat.value}</p>
                </div>
                <div className='rounded-lg bg-primary/10 p-2'>
                    <stat.icon className='size-4 text-primary' />
                </div>
            </div>
            <div className='mt-2 flex items-center gap-2'>
                {stat.trend === "up" && <TrendingUp className='size-3 text-primary' />}
                {stat.trend === "down" && <TrendingDown className='size-3 text-destructive' />}
                <span
                    className={`text-xs font-medium ${stat.trend === "up" ? "text-primary" : stat.trend === "down" ? "text-destructive" : "text-muted-foreground"}`}
                >
                    {stat.trendValue}
                </span>
                <span className='text-xs text-muted-foreground'>{stat.detail}</span>
            </div>
        </div>
    );
}

function NodeBreakdownPanel({ stats }: { stats: StatCardData[] }) {
    const healthStat = stats.find((s) => s.healthNodes && s.healthNodes.length > 0);
    const nodeSections = stats.filter((s) => s.nodes && s.nodes.length > 0);

    if (!healthStat && nodeSections.length === 0) return null;

    return (
        <div className='grid gap-4 sm:grid-cols-2 lg:grid-cols-3'>
            {/* Node Health */}
            {healthStat?.healthNodes && (
                <div className='space-y-1.5'>
                    <p className='text-[11px] font-medium tracking-wider text-muted-foreground uppercase'>Node Health</p>
                    {healthStat.healthNodes.map((node) => {
                        const isReady = node.status === "ready";
                        const isActive = node.availability === "active";
                        return (
                            <div key={node.name} className='flex items-center gap-2.5'>
                                <span
                                    className={`size-1.5 shrink-0 rounded-full ${
                                        isReady && isActive ? "bg-primary" : isReady ? "bg-warning" : "bg-destructive"
                                    }`}
                                />
                                <span className='flex-1 truncate font-mono text-xs'>{node.name}</span>
                                <span className={`text-[10px] font-medium ${isReady ? "text-primary" : "text-destructive"}`}>{node.status}</span>
                                <span className='rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground'>
                                    {node.role}
                                    {node.leader ? " \u2605" : ""}
                                </span>
                            </div>
                        );
                    })}
                </div>
            )}

            {/* CPU / Memory per-node bars */}
            {nodeSections.map((stat) => (
                <div key={stat.label} className='space-y-2'>
                    <p className='text-[11px] font-medium tracking-wider text-muted-foreground uppercase'>{stat.label} per node</p>
                    {stat.nodes!.map((node) => {
                        const pct = node.max > 0 ? (node.value / node.max) * 100 : 0;
                        return (
                            <div key={node.name}>
                                <div className='flex items-center justify-between text-xs'>
                                    <span className='truncate font-mono text-muted-foreground'>{node.name}</span>
                                    <span className='font-mono'>{Math.round(pct)}%</span>
                                </div>
                                <div className='mt-1 h-1.5 overflow-hidden rounded-full bg-muted'>
                                    <div
                                        className={`h-full rounded-full transition-all ${pct > 80 ? "bg-destructive" : pct > 60 ? "bg-warning" : "bg-primary"}`}
                                        style={{ width: `${Math.min(pct, 100)}%` }}
                                    />
                                </div>
                            </div>
                        );
                    })}
                </div>
            ))}
        </div>
    );
}

export function StatsCards({ nodes, agents, taskStats }: StatsCardsProps) {
    const [expanded, setExpanded] = useState(false);
    const panelRef = useRef<HTMLDivElement>(null);
    const [panelHeight, setPanelHeight] = useState(0);

    const nodeCount = nodes.length;
    const readyNodes = nodes.filter((n) => n.status === "ready").length;
    const managers = nodes.filter((n) => n.role === "manager").length;

    const hasMetrics = agents.length > 0;
    const avgCpu = hasMetrics ? agents.reduce((sum, a) => sum + a.node.cpuPercent, 0) / agents.length : 0;
    const totalMemUsed = agents.reduce((sum, a) => sum + a.node.memoryUsed, 0);
    const totalMemTotal = agents.reduce((sum, a) => sum + a.node.memoryTotal, 0);
    const memPct = totalMemTotal > 0 ? (totalMemUsed / totalMemTotal) * 100 : 0;

    const cpuNodes = agents.map((a) => ({
        name: a.nodeId.slice(0, 12),
        value: a.node.cpuPercent,
        max: 100,
    }));
    const memNodes = agents.map((a) => ({
        name: a.nodeId.slice(0, 12),
        value: a.node.memoryUsed,
        max: a.node.memoryTotal,
    }));

    const stats: StatCardData[] = [
        {
            label: "Cluster Nodes",
            value: String(nodeCount),
            icon: Server,
            trend: readyNodes === nodeCount ? "up" : "down",
            trendValue: `${readyNodes}/${nodeCount} ready`,
            detail: `${managers} manager${managers !== 1 ? "s" : ""}`,
            healthNodes: nodes.map((n) => ({
                name: n.id.slice(0, 12),
                role: n.role,
                status: n.status,
                availability: n.availability,
                leader: n.leader,
            })),
        },
        {
            label: "CPU Usage",
            value: hasMetrics ? `${Math.round(avgCpu)}%` : "N/A",
            icon: Cpu,
            trend: avgCpu < 60 ? "up" : avgCpu < 80 ? "neutral" : "down",
            trendValue: hasMetrics ? `${agents.length} node${agents.length !== 1 ? "s" : ""} reporting` : "No data",
            detail: hasMetrics ? "avg across cluster" : "",
            nodes: cpuNodes,
        },
        {
            label: "Memory",
            value: hasMetrics ? `${Math.round(memPct)}%` : "N/A",
            icon: MemoryStick,
            trend: memPct < 60 ? "up" : memPct < 80 ? "neutral" : "down",
            trendValue: hasMetrics ? `${formatBytes(totalMemUsed)} / ${formatBytes(totalMemTotal)}` : "No data",
            detail: hasMetrics ? "total cluster memory" : "",
            nodes: memNodes,
        },
        {
            label: "Tasks",
            value: `${taskStats.running}`,
            icon: ListChecks,
            trend: taskStats.failed > 0 ? "down" : "up",
            trendValue: taskStats.failed > 0 ? `${taskStats.failed} failed` : `${taskStats.running}/${taskStats.desiredRunning} running`,
            detail: `${taskStats.total} total`,
        },
    ];

    const hasExpandableData = stats.some((s) => (s.healthNodes && s.healthNodes.length > 0) || (s.nodes && s.nodes.length > 0));

    useEffect(() => {
        if (panelRef.current) {
            setPanelHeight(panelRef.current.scrollHeight);
        }
    }, [expanded, nodes, agents]);

    return (
        <div className='rounded-xl border border-border bg-card/50 p-4'>
            {/* Cards grid */}
            <div className='grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4'>
                {stats.map((stat, i) => (
                    <StatCard key={stat.label} stat={stat} delay={i * 50} borderClass={statBorders[i] ?? ""} />
                ))}
            </div>

            {/* Sliding panel + toggle handle */}
            {hasExpandableData && (
                <>
                    {/* Sliding panel */}
                    <div
                        className='overflow-hidden transition-all duration-300 ease-in-out'
                        style={{ maxHeight: expanded ? `${panelHeight}px` : "0px", opacity: expanded ? 1 : 0 }}
                    >
                        <div ref={panelRef} className='py-4 border-t mt-4'>
                            <NodeBreakdownPanel stats={stats} />
                        </div>
                    </div>

                    {/* Toggle handle */}
                    <button
                        onClick={() => setExpanded(!expanded)}
                        className='flex w-full items-center justify-center gap-1.5 rounded-lg border border-dashed border-border py-3 mt-4 text-[11px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground'
                    >
                        <ChevronDown className='size-4 transition-transform duration-300' style={{ transform: expanded ? "rotate(180deg)" : "rotate(0deg)" }} />
                        {expanded ? "Hide" : "Show"} per-node breakdown
                    </button>
                </>
            )}
        </div>
    );
}
