import { Link, useMatches } from "@tanstack/react-router";
import { Activity, Box, Database, FileText, Globe, HardDrive, KeyRound, Layers, LayoutDashboard, Server, Settings, Terminal } from "lucide-react";
import { ClusterSwitcher } from "@/components/cluster-switcher";
import { useServices } from "@/hooks/use-services";
import { useNodes } from "@/hooks/use-nodes";
import { useStacks } from "@/hooks/use-stacks";
import { useTaskStats } from "@/hooks/use-task-stats";
import { useClusterMetrics } from "@/hooks/use-cluster-metrics";
import { useImages } from "@/hooks/use-images";
import { useSecrets } from "@/hooks/use-secrets";
import { useConfigs } from "@/hooks/use-configs";

const mainNav = [
    { label: "Overview", to: "/" as const, icon: LayoutDashboard },
    { label: "Nodes", to: "/nodes" as const, icon: Server, countKey: "nodes" as const },
    { label: "Stacks", to: "/stacks" as const, icon: Layers, countKey: "stacks" as const },
    { label: "Services", to: "/services" as const, icon: Activity, countKey: "services" as const },
    { label: "Containers", to: "/tasks" as const, icon: Box, countKey: "tasks" as const },
    { label: "Networking", to: "/networks" as const, icon: Globe },
    { label: "Volumes", to: "/volumes" as const, icon: Database },
    { label: "Images", to: "/images" as const, icon: HardDrive, countKey: "images" as const },
    { label: "Secrets", to: "/secrets" as const, icon: KeyRound, countKey: "secrets" as const },
    { label: "Configs", to: "/configs" as const, icon: FileText, countKey: "configs" as const },
    { label: "Logs", to: "/logs" as const, icon: Terminal },
];

const bottomNav = [{ label: "Settings", to: "/settings" as const, icon: Settings }];

export function SidebarNav() {
    const matches = useMatches();
    const currentPath = matches[matches.length - 1]?.fullPath ?? "/";

    const { data: services } = useServices();
    const { data: nodes } = useNodes();
    const { data: stacks } = useStacks();
    const { data: taskStats } = useTaskStats();
    const { data: metricsData } = useClusterMetrics();
    const { data: imagesData } = useImages();
    const { data: secrets } = useSecrets();
    const { data: configs } = useConfigs();

    const counts: Record<string, number> = {
        services: services?.length ?? 0,
        nodes: nodes?.length ?? 0,
        stacks: stacks?.length ?? 0,
        tasks: taskStats?.running ?? 0,
        images: imagesData?.nodes?.reduce((sum, n) => sum + n.images.length, 0) ?? 0,
        secrets: secrets?.length ?? 0,
        configs: configs?.length ?? 0,
    };

    function isActive(to: string): boolean {
        if (to === "/") return currentPath === "/";
        return currentPath.startsWith(to);
    }

    // Compute resource usage for footer gauge
    const agents = metricsData?.agents ?? [];
    let resourcePct = 0;
    if (agents.length > 0) {
        const avgCpu = agents.reduce((sum, a) => sum + a.node.cpuPercent, 0) / agents.length;
        const totalMemUsed = agents.reduce((sum, a) => sum + a.node.memoryUsed, 0);
        const totalMemTotal = agents.reduce((sum, a) => sum + a.node.memoryTotal, 0);
        const memPct = totalMemTotal > 0 ? (totalMemUsed / totalMemTotal) * 100 : 0;
        resourcePct = Math.round((avgCpu + memPct) / 2);
    }

    return (
        <aside className='flex w-60 shrink-0 flex-col border-r border-border bg-card'>
            {/* Cluster Switcher */}
            <div className='border-b border-border p-3'>
                <ClusterSwitcher />
            </div>

            {/* Main Navigation */}
            <nav className='flex-1 overflow-y-auto px-3 py-3'>
                <div className='space-y-0.5'>
                    {mainNav.map((item) => {
                        const active = isActive(item.to);
                        const count = item.countKey ? counts[item.countKey] : undefined;

                        return (
                            <Link
                                key={item.to}
                                to={item.to}
                                className={`flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors ${
                                    active ? 'bg-primary/10 text-primary' : 'text-muted-foreground hover:bg-accent hover:text-foreground'
                                }`}
                            >
                                <item.icon className='size-4' />
                                <span className='flex-1'>{item.label}</span>
                                {count !== undefined && count > 0 && (
                                    <span
                                        className={`rounded-full px-1.5 py-0.5 font-mono text-[10px] leading-none ${
                                            active ? 'bg-primary/20 text-primary' : 'bg-muted text-muted-foreground'
                                        }`}
                                    >
                                        {count}
                                    </span>
                                )}
                            </Link>
                        );
                    })}
                </div>
            </nav>

            {/* Bottom Navigation */}
            <div className='border-t border-border px-3 py-3'>
                <div className='space-y-0.5'>
                    {bottomNav.map((item) => {
                        const active = isActive(item.to);
                        return (
                            <Link
                                key={item.to}
                                to={item.to}
                                className={`flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors ${
                                    active ? 'bg-primary/10 text-primary' : 'text-muted-foreground hover:bg-accent hover:text-foreground'
                                }`}
                            >
                                <item.icon className='size-4' />
                                <span>{item.label}</span>
                            </Link>
                        );
                    })}
                </div>
            </div>

            {/* Resource Usage Footer */}
            {agents.length > 0 && (
                <div className='border-t border-border px-4 py-3'>
                    <div className='flex items-center justify-between text-[11px] text-muted-foreground'>
                        <span>Resource Usage</span>
                        <span className='font-mono'>{resourcePct}%</span>
                    </div>
                    <div className='mt-1.5 h-1.5 overflow-hidden rounded-full bg-muted'>
                        <div
                            className={`h-full rounded-full transition-all ${
                                resourcePct > 80 ? "bg-destructive" : resourcePct > 60 ? "bg-warning" : "bg-primary"
                            }`}
                            style={{ width: `${Math.min(resourcePct, 100)}%` }}
                        />
                    </div>
                </div>
            )}
        </aside>
    );
}
