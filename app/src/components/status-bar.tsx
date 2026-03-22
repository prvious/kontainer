import { useNavigate } from "@tanstack/react-router";
import { GitBranch, Container, Cpu, MemoryStick, Layers, TriangleAlert, ScrollText } from "lucide-react";
import { useTaskStats } from "@/hooks/use-task-stats";
import { useClusterMetrics } from "@/hooks/use-cluster-metrics";
import { TerminalPopover } from "@/components/terminal-popover";

function Separator() {
    return <span className='h-full w-px bg-border' />;
}

function Segment({
    icon: Icon,
    iconColor,
    label,
    textColor,
    onClick,
    testId,
    valueTestId,
}: {
    icon: typeof Cpu;
    iconColor: string;
    label: string;
    textColor?: string;
    onClick?: () => void;
    testId?: string;
    valueTestId?: string;
}) {
    const Wrapper = onClick ? "button" : "div";
    return (
        <Wrapper
            className={`flex h-full items-center gap-[5px] px-2.5 ${textColor ?? "text-text-secondary"} ${onClick ? 'transition-colors hover:text-foreground' : ""}`}
            onClick={onClick}
            data-testid={testId}
        >
            <Icon className={`size-[11px] ${iconColor}`} />
            <span className='font-mono text-[10px] font-medium' data-testid={valueTestId}>
                {label}
            </span>
        </Wrapper>
    );
}

export function StatusBar() {
    const navigate = useNavigate();
    const { data: taskStats, isLoading: taskStatsLoading } = useTaskStats();
    const { data: metrics, isLoading: metricsLoading } = useClusterMetrics();

    // Metrics
    const agents = metrics?.agents ?? [];
    const avgCpu = agents.length > 0 ? Math.round(agents.reduce((sum, a) => sum + a.node.cpuPercent, 0) / agents.length) : null;
    const totalMem = agents.reduce((sum, a) => sum + a.node.memoryUsed, 0);
    const totalMemMax = agents.reduce((sum, a) => sum + a.node.memoryTotal, 0);
    const memDisplay = metricsLoading || totalMemMax === 0 ? "--" : `${(totalMem / 1e9).toFixed(1)}/${(totalMemMax / 1e9).toFixed(0)}G`;

    // Tasks
    const taskCount = taskStatsLoading ? "--" : `${taskStats?.running ?? 0} TASKS`;
    const taskColor = (taskStats?.failed ?? 0) > 0 ? "text-accent-red" : undefined;

    // CPU display
    const cpuDisplay = metricsLoading || avgCpu === null ? "--" : `CPU ${avgCpu}%`;

    // Alerts (failed tasks)
    const alertCount = taskStats?.failed ?? 0;

    return (
        <div className='flex h-7 shrink-0 items-center border-t border-border bg-statusbar-bg/95 text-xs'>
            {/* Left segments */}
            <div className='flex h-full items-center'>
                <Segment icon={GitBranch} iconColor='text-muted-foreground' label='main' textColor='text-muted-foreground' /> {/* TODO: wire to agent data */}
                <Separator />
                <Segment icon={Container} iconColor='text-muted-foreground' label='Docker --' textColor='text-muted-foreground' />{" "}
                {/* TODO: wire to agent data */}
                <Separator />
                <Segment
                    icon={Cpu}
                    iconColor='text-accent-cyan'
                    label={cpuDisplay}
                    testId='status-cpu'
                    valueTestId='cpu-value'
                    onClick={() => navigate({ to: "/" })}
                />
                <Separator />
                <Segment
                    icon={MemoryStick}
                    iconColor='text-accent-purple'
                    label={`MEM ${memDisplay}`}
                    testId='status-mem'
                    valueTestId='mem-value'
                    onClick={() => navigate({ to: "/" })}
                />
                <Separator />
                <Segment
                    icon={Layers}
                    iconColor='text-accent-orange'
                    label={taskCount}
                    textColor={taskColor}
                    testId='status-tasks'
                    onClick={() => navigate({ to: "/tasks" })}
                />
                <Separator />
                {alertCount > 0 && (
                    <Segment
                        icon={TriangleAlert}
                        iconColor='text-accent-red'
                        label={`${alertCount} ALERT${alertCount > 1 ? "S" : ""}`}
                        textColor='text-accent-red'
                    />
                )}
            </div>

            {/* Spacer */}
            <div className='flex-1' />

            {/* Right segments */}
            <div className='flex h-full items-center'>
                {/* Terminal sessions */}
                <Separator />
                <TerminalPopover />
                <Separator />
                <Segment icon={ScrollText} iconColor='text-muted-foreground' label='-- STREAMS' textColor='text-muted-foreground' />{" "}
                {/* TODO: wire to log stream count */}
                <Separator />
                <div className='flex h-full items-center gap-[5px] px-2.5'>
                    <span className='size-1.5 rounded-full bg-muted-foreground' />
                    <span className='font-mono text-[10px] font-medium text-muted-foreground'>WS --</span> {/* TODO: wire to WebSocket state */}
                </div>
            </div>
        </div>
    );
}
