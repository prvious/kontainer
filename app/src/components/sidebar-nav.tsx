import { Link, useMatches } from "@tanstack/react-router";
import { Activity, Box, HardDrive, FileStack, LayoutDashboard, Layers, Network, Server, Settings, ScrollText } from "lucide-react";
import { useNodes } from "@/hooks/use-nodes";
import { useTaskStats } from "@/hooks/use-task-stats";

type NavItem = {
    label: string;
    to: string;
    icon: typeof LayoutDashboard;
    countKey?: string;
    badgeColor?: string;
};
type NavSection = { heading: string; items: NavItem[] };

const topSections: NavSection[] = [
    {
        heading: "INFRASTRUCTURE",
        items: [
            { label: "OVERVIEW", to: "/", icon: LayoutDashboard },
            { label: "CONTAINERS", to: "/tasks", icon: Box, countKey: "tasks", badgeColor: "bg-accent-green-20 text-accent-green" },
            { label: "IMAGES", to: "/images", icon: Layers },
            { label: "VOLUMES", to: "/volumes", icon: HardDrive },
            { label: "NETWORKS", to: "/networks", icon: Network },
        ],
    },
    {
        heading: "ORCHESTRATION",
        items: [
            { label: "STACKS", to: "/stacks", icon: FileStack },
            { label: "NODES", to: "/nodes", icon: Server, countKey: "nodes", badgeColor: "bg-accent-orange-20 text-accent-orange" },
            { label: "SERVICES", to: "/services", icon: Activity },
        ],
    },
];

const bottomSection: NavSection = {
    heading: "SYSTEM",
    items: [
        { label: "LOGS", to: "/logs", icon: ScrollText },
        { label: "SETTINGS", to: "/settings", icon: Settings },
    ],
};

export function SidebarNav() {
    const matches = useMatches();
    const currentPath = matches[matches.length - 1]?.fullPath ?? "/";

    const { data: nodes } = useNodes();
    const { data: taskStats } = useTaskStats();

    const counts: Record<string, number> = {
        tasks: taskStats?.running ?? 0,
        nodes: nodes?.length ?? 0,
    };

    function isActive(to: string): boolean {
        if (to === "/") return currentPath === "/";
        return currentPath.startsWith(to);
    }

    function renderSection(section: NavSection) {
        return (
            <div key={section.heading}>
                <p className='px-3 font-mono text-[9px] font-semibold tracking-[1px] text-muted-foreground'>// {section.heading}</p>
                <div className='mt-1 space-y-0.5'>
                    {section.items.map((item) => {
                        const active = isActive(item.to);
                        const count = item.countKey ? counts[item.countKey] : undefined;

                        return (
                            <Link
                                key={item.to}
                                to={item.to}
                                className={`flex h-[34px] items-center gap-2.5 rounded px-3 font-mono text-xs font-medium tracking-[0.5px] transition-colors ${
                                    active ? 'bg-sidebar-accent text-foreground' : 'text-text-secondary hover:text-foreground'
                                }`}
                            >
                                <item.icon className='size-4 shrink-0 text-muted-foreground' />
                                <span className='flex-1'>{item.label}</span>
                                {count !== undefined && count > 0 && (
                                    <span
                                        className={`rounded px-1.5 py-0.5 font-mono text-[10px] leading-none ${
                                            item.badgeColor ?? 'bg-sidebar-accent text-muted-foreground'
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
        );
    }

    return (
        <aside className='flex w-[220px] shrink-0 flex-col border-r border-border bg-sidebar/90'>
            {/* Top: main navigation */}
            <nav className='flex-1 space-y-4 overflow-y-auto px-2 py-3'>{topSections.map(renderSection)}</nav>

            {/* Bottom: system + agent info */}
            <div className='space-y-3 px-2 pb-3'>
                {renderSection(bottomSection)}
                {/* TODO: wire to agent health endpoint */}
                <div className='rounded border border-border bg-bg-elevated px-3 py-2.5'>
                    <p className='font-mono text-[9px] font-bold tracking-[0.5px] text-muted-foreground'>AGENT --</p>
                    <p className='font-mono text-[9px] font-medium tracking-[0.5px] text-muted-foreground'>uptime: --</p>
                </div>
            </div>
        </aside>
    );
}
