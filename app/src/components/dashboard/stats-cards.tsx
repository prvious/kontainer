import { useState } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { ChevronDown, ChevronUp, Cpu, MemoryStick, Network, Server, TrendingDown, TrendingUp } from "lucide-react";
import type { NodeItem } from "@/types/nodes";
import type { AgentMetricsSnapshot } from "@/types/metrics";

interface StatsCardsProps {
    nodes: NodeItem[];
    agents: AgentMetricsSnapshot[];
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

function StatCard({ stat, delay }: { stat: StatCardData; delay: number }) {
    const [expanded, setExpanded] = useState(false);
    const hasNodes = stat.nodes && stat.nodes.length > 0;
    const hasHealthNodes = stat.healthNodes && stat.healthNodes.length > 0;
    const expandable = hasNodes || hasHealthNodes;

    return (
        <Card className='animate-fade-in-up overflow-hidden pt-0' style={{ animationDelay: `${delay}ms` }}>
            <CardContent className='p-4'>
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
            </CardContent>

            {/* Expandable breakdown */}
            {expandable && (
                <>
                    <button
                        onClick={() => setExpanded(!expanded)}
                        className='flex w-full items-center justify-center gap-1 border-t border-border py-1.5 text-[11px] text-muted-foreground transition-colors hover:bg-accent'
                    >
                        {expanded ? <ChevronUp className='size-3' /> : <ChevronDown className='size-3' />}
                        {expanded ? "Hide" : "Show"} per-node
                    </button>
                    {expanded && hasHealthNodes && (
                        <div className='space-y-0 border-t border-border'>
                            {stat.healthNodes!.map((node) => {
                                const isReady = node.status === "ready";
                                const isActive = node.availability === "active";
                                return (
                                    <div key={node.name} className='flex items-center gap-3 px-4 py-2 not-last:border-b not-last:border-border'>
                                        <span
                                            className={`size-2 shrink-0 rounded-full ${
                                                isReady && isActive ? "bg-primary" : isReady ? "bg-warning" : "bg-destructive"
                                            }`}
                                        />
                                        <div className='flex min-w-0 flex-1 items-center justify-between gap-2'>
                                            <span className='truncate font-mono text-xs'>{node.name}</span>
                                            <div className='flex shrink-0 items-center gap-2'>
                                                <span className={`text-[10px] font-medium ${isReady ? "text-primary" : "text-destructive"}`}>
                                                    {node.status}
                                                </span>
                                                <span className='rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground'>
                                                    {node.role}
                                                    {node.leader ? " ★" : ""}
                                                </span>
                                            </div>
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    )}
                    {expanded && hasNodes && (
                        <div className='space-y-2 border-t border-border px-4 py-3'>
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
                                                className={`h-full rounded-full transition-all ${
                                                    pct > 80 ? "bg-destructive" : pct > 60 ? "bg-warning" : "bg-primary"
                                                }`}
                                                style={{ width: `${Math.min(pct, 100)}%` }}
                                            />
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    )}
                </>
            )}
        </Card>
    );
}

export function StatsCards({ nodes, agents }: StatsCardsProps) {
    const nodeCount = nodes.length;
    const readyNodes = nodes.filter((n) => n.status === "ready").length;
    const managers = nodes.filter((n) => n.role === "manager").length;

    // Compute metrics from agents
    const hasMetrics = agents.length > 0;
    const avgCpu = hasMetrics ? agents.reduce((sum, a) => sum + a.node.cpuPercent, 0) / agents.length : 0;
    const totalMemUsed = agents.reduce((sum, a) => sum + a.node.memoryUsed, 0);
    const totalMemTotal = agents.reduce((sum, a) => sum + a.node.memoryTotal, 0);
    const memPct = totalMemTotal > 0 ? (totalMemUsed / totalMemTotal) * 100 : 0;

    const cpuNodes = agents.map((a) => ({
        name: a.hostname,
        value: a.node.cpuPercent,
        max: 100,
    }));
    const memNodes = agents.map((a) => ({
        name: a.hostname,
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
                name: n.hostname,
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
            label: "Network I/O",
            value: "—",
            icon: Network,
            trend: "neutral",
            trendValue: "Coming soon",
            detail: "not yet collected",
        },
    ];

    return (
        <div className='grid gap-4 sm:grid-cols-2 lg:grid-cols-4'>
            {stats.map((stat, i) => (
                <StatCard key={stat.label} stat={stat} delay={i * 50} />
            ))}
        </div>
    );
}
