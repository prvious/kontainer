import { useNavigate } from "@tanstack/react-router";
import { Server, ListChecks, Cpu, MemoryStick } from "lucide-react";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { useAuth } from "@/hooks/use-auth";
import { useNodes } from "@/hooks/use-nodes";
import { useTaskStats } from "@/hooks/use-task-stats";
import { useClusterMetrics } from "@/hooks/use-cluster-metrics";

function resourceColor(pct: number): string {
    if (pct > 90) return "text-destructive";
    if (pct >= 70) return "text-warning";
    return "";
}

export function StatusBar() {
    const navigate = useNavigate();
    const { profiles, activeProfileId } = useAuth();
    const { data: nodes, isLoading: nodesLoading } = useNodes();
    const { data: taskStats, isLoading: taskStatsLoading } = useTaskStats();
    const { data: metrics, isLoading: metricsLoading } = useClusterMetrics();

    const activeProfile = profiles.find((p) => p.id === activeProfileId);
    const clusterName = activeProfile?.label ?? "--";

    // Nodes
    const totalNodes = nodes?.length ?? 0;
    const readyNodes = nodes?.filter((n) => n.status === "ready").length ?? 0;
    const allNodesReady = totalNodes > 0 && readyNodes === totalNodes;
    const nodeLabel = nodesLoading ? "--" : allNodesReady ? `${totalNodes} nodes` : `${readyNodes}/${totalNodes} nodes`;
    const nodeWarning = !nodesLoading && totalNodes > 0 && readyNodes < totalNodes;

    // Tasks
    const running = taskStats?.running ?? 0;
    const desired = taskStats?.desiredRunning ?? 0;
    const failed = taskStats?.failed ?? 0;
    const taskLabel = taskStatsLoading ? "--" : `${running}/${desired} tasks`;
    const taskWarning = !taskStatsLoading && desired > 0 && running < desired;
    const taskDanger = !taskStatsLoading && failed > 0;

    // Metrics (averaged across agents)
    const agents = metrics?.agents ?? [];
    const avgCpu = agents.length > 0 ? Math.round(agents.reduce((sum, a) => sum + a.node.cpuPercent, 0) / agents.length) : null;
    const avgMem = agents.length > 0 ? Math.round(agents.reduce((sum, a) => sum + a.node.memoryPercent, 0) / agents.length) : null;

    const cpuDisplay = metricsLoading || avgCpu === null ? "--" : `${avgCpu}%`;
    const memDisplay = metricsLoading || avgMem === null ? "--" : `${avgMem}%`;

    const cpuColor = avgCpu !== null ? resourceColor(avgCpu) : "";
    const memColor = avgMem !== null ? resourceColor(avgMem) : "";

    function taskColorClass(): string {
        if (taskDanger) return "text-destructive";
        if (taskWarning) return "text-warning";
        return "";
    }

    return (
        <TooltipProvider>
            <div className='flex h-7 shrink-0 items-center gap-1.5 border-t border-border bg-card px-3 text-xs text-muted-foreground'>
                {/* Cluster name */}
                <div className='flex items-center gap-1.5'>
                    <span className='animate-pulse-dot size-1.5 shrink-0 rounded-full bg-primary' />
                    <span className='font-medium'>{clusterName}</span>
                </div>

                <span className='h-3 w-px bg-border' />

                {/* Nodes */}
                <Tooltip>
                    <TooltipTrigger
                        render={(props) => (
                            <button
                                {...props}
                                data-testid='status-nodes'
                                className={`flex items-center gap-1 hover:text-foreground ${nodeWarning ? "text-warning" : ""}`}
                                onClick={() => navigate({ to: "/nodes" })}
                            >
                                <Server className='size-3' />
                                <span>{nodeLabel}</span>
                            </button>
                        )}
                    />
                    <TooltipContent>View nodes</TooltipContent>
                </Tooltip>

                <span className='h-3 w-px bg-border' />

                {/* Tasks */}
                <Tooltip>
                    <TooltipTrigger
                        render={(props) => (
                            <button
                                {...props}
                                data-testid='status-tasks'
                                className={`flex items-center gap-1 hover:text-foreground ${taskColorClass()}`}
                                onClick={() => navigate({ to: "/tasks" })}
                            >
                                <ListChecks className='size-3' />
                                <span>{taskLabel}</span>
                            </button>
                        )}
                    />
                    <TooltipContent>View tasks</TooltipContent>
                </Tooltip>

                {/* Spacer */}
                <div className='flex-1' />

                {/* CPU */}
                <Tooltip>
                    <TooltipTrigger
                        render={(props) => (
                            <button
                                {...props}
                                data-testid='status-cpu'
                                className={`flex items-center gap-1 hover:text-foreground ${cpuColor}`}
                                onClick={() => navigate({ to: "/" })}
                            >
                                <Cpu className='size-3' />
                                <span data-testid='cpu-value'>{cpuDisplay}</span>
                            </button>
                        )}
                    />
                    <TooltipContent>View dashboard</TooltipContent>
                </Tooltip>

                <span className='h-3 w-px bg-border' />

                {/* MEM */}
                <Tooltip>
                    <TooltipTrigger
                        render={(props) => (
                            <button
                                {...props}
                                data-testid='status-mem'
                                className={`flex items-center gap-1 hover:text-foreground ${memColor}`}
                                onClick={() => navigate({ to: "/" })}
                            >
                                <MemoryStick className='size-3' />
                                <span data-testid='mem-value'>{memDisplay}</span>
                            </button>
                        )}
                    />
                    <TooltipContent>View dashboard</TooltipContent>
                </Tooltip>
            </div>
        </TooltipProvider>
    );
}
