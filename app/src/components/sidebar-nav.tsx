import { Link, useMatches } from "@tanstack/react-router";
import { Activity, Box, Database, FileText, Globe, HardDrive, KeyRound, Layers, LayoutDashboard, Server, Settings, Terminal } from "lucide-react";
import { ClusterSwitcher } from "@/components/cluster-switcher";
import { useServices } from "@/hooks/use-services";
import { useNodes } from "@/hooks/use-nodes";
import { useStacks } from "@/hooks/use-stacks";
import { useTaskStats } from "@/hooks/use-task-stats";

import { useImages } from "@/hooks/use-images";
import { useSecrets } from "@/hooks/use-secrets";
import { useConfigs } from "@/hooks/use-configs";

type NavItem = { label: string; to: string; icon: typeof LayoutDashboard; countKey?: string };
type NavSection = { heading?: string; items: NavItem[] };

const navSections: NavSection[] = [
    {
        heading: "Cluster",
        items: [
            { label: "Overview", to: "/" as const, icon: LayoutDashboard },
            { label: "Nodes", to: "/nodes" as const, icon: Server, countKey: "nodes" as const },
        ],
    },
    {
        heading: "Workloads",
        items: [
            { label: "Stacks", to: "/stacks" as const, icon: Layers, countKey: "stacks" as const },
            { label: "Services", to: "/services" as const, icon: Activity, countKey: "services" as const },
            { label: "Containers", to: "/tasks" as const, icon: Box, countKey: "tasks" as const },
        ],
    },
    {
        heading: "Infrastructure",
        items: [
            { label: "Networks", to: "/networks" as const, icon: Globe },
            { label: "Volumes", to: "/volumes" as const, icon: Database },
            { label: "Images", to: "/images" as const, icon: HardDrive, countKey: "images" as const },
        ],
    },
    {
        heading: "Configuration",
        items: [
            { label: "Secrets", to: "/secrets" as const, icon: KeyRound, countKey: "secrets" as const },
            { label: "Configs", to: "/configs" as const, icon: FileText, countKey: "configs" as const },
        ],
    },
    {
        heading: "Observability",
        items: [{ label: "Logs", to: "/logs" as const, icon: Terminal }],
    },
];

export function SidebarNav() {
    const matches = useMatches();
    const currentPath = matches[matches.length - 1]?.fullPath ?? "/";

    const { data: services } = useServices();
    const { data: nodes } = useNodes();
    const { data: stacks } = useStacks();
    const { data: taskStats } = useTaskStats();
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

    return (
        <aside data-tauri-drag-region className='flex w-60 shrink-0 flex-col bg-sidebar pt-(--titlebar-inset)'>
            {/* Main Navigation */}
            <nav className='flex-1 overflow-y-auto px-3 py-3'>
                <div className='space-y-4'>
                    {navSections.map((section) => (
                        <div key={section.heading}>
                            {section.heading && (
                                <p className='mb-1 px-3 text-[11px] font-semibold tracking-wider text-sidebar-foreground/40 uppercase'>{section.heading}</p>
                            )}
                            <div className='space-y-0.5'>
                                {section.items.map((item) => {
                                    const active = isActive(item.to);
                                    const count = item.countKey ? counts[item.countKey] : undefined;

                                    return (
                                        <Link
                                            key={item.to}
                                            to={item.to}
                                            className={`flex items-center gap-3 rounded-md px-3 py-1.5 text-sm font-medium transition-colors ${
                                                active
                                                    ? 'bg-sidebar-accent text-sidebar-primary'
                                                    : 'text-sidebar-foreground/60 hover:bg-sidebar-accent hover:text-sidebar-foreground'
                                            }`}
                                        >
                                            <item.icon className='size-4' />
                                            <span className='flex-1'>{item.label}</span>
                                            {count !== undefined && count > 0 && (
                                                <span
                                                    className={`rounded-full px-1.5 py-0.5 font-mono text-[10px] leading-none ${
                                                        active ? 'bg-sidebar-primary/20 text-sidebar-primary' : 'bg-sidebar-accent text-sidebar-foreground/60'
                                                    }`}
                                                >
                                                    {count}
                                                </span>
                                            )}
                                        </Link>
                                    );
                                })}
                            </div>
                        </div>
                    ))}
                </div>
            </nav>

            {/* Bottom: Settings + Cluster Switcher */}
            <div className='space-y-0.5 px-3 pb-3'>
                <Link
                    to='/settings'
                    className={`flex items-center gap-3 rounded-md px-3 py-1.5 text-sm font-medium transition-colors ${
                        isActive("/settings")
                            ? 'bg-sidebar-accent text-sidebar-primary'
                            : 'text-sidebar-foreground/60 hover:bg-sidebar-accent hover:text-sidebar-foreground'
                    }`}
                >
                    <Settings className='size-4' />
                    <span>Settings</span>
                </Link>
                <ClusterSwitcher />
            </div>
        </aside>
    );
}
