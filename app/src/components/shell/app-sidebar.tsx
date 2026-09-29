import { Link, type LinkProps } from "@tanstack/react-router";
import { Activity, Box, FileStack, HardDrive, Layers, LayoutDashboard, Network, ScrollText, Server, Settings, Terminal, type LucideIcon } from "lucide-react";
import { Sidebar, SidebarContent, SidebarFooter, SidebarGroup, SidebarGroupLabel, SidebarMenu, SidebarMenuItem } from "@/components/ui/sidebar";
import { uptime } from "@/components/containers/container-format";
import { useNow } from "@/hooks/use-now";
import { cn } from "@/lib/utils";

export interface SidebarBadges {
    /** Total containers; hidden at zero. */
    containers?: number;
    /** Nodes that are down or degraded; renders "N DOWN", hidden at zero. */
    nodesDown?: number;
}

export interface AgentInfo {
    version: string;
    /** When the controller started (RFC 3339); uptime counts from it. */
    startedAt?: string;
}

interface NavItem {
    label: string;
    icon: LucideIcon;
    to: LinkProps["to"];
    exact?: boolean;
    badge?: (badges: SidebarBadges) => string | null;
}

const count = (value: number | undefined) => (value ? String(value) : null);

/** The Agent Info box's uptime line, on the shared one-second clock. */
function AgentUptime({ startedAt }: { startedAt?: string }) {
    const now = useNow();
    return <span className='font-medium text-muted-foreground'>uptime: {startedAt ? uptime((now - Date.parse(startedAt)) / 1_000) : "—"}</span>;
}

const NAV_SECTIONS: { label: string; items: NavItem[] }[] = [
    {
        label: "// INFRASTRUCTURE",
        items: [
            { label: "Overview", icon: LayoutDashboard, to: "/", exact: true },
            { label: "Containers", icon: Box, to: "/containers", badge: (badges) => count(badges.containers) },
            { label: "Images", icon: Layers, to: "/images" },
            { label: "Volumes", icon: HardDrive, to: "/volumes" },
            { label: "Networks", icon: Network, to: "/networks" },
        ],
    },
    {
        label: "// ORCHESTRATION",
        items: [
            { label: "Stacks", icon: FileStack, to: "/stacks" },
            { label: "Nodes", icon: Server, to: "/nodes", badge: (badges) => (badges.nodesDown ? `${badges.nodesDown} DOWN` : null) },
            { label: "Services", icon: Activity, to: "/services" },
        ],
    },
    {
        label: "// SYSTEM",
        items: [
            { label: "Exec", icon: Terminal, to: "/exec" },
            { label: "Logs", icon: ScrollText, to: "/logs" },
            { label: "Settings", icon: Settings, to: "/settings" },
        ],
    },
];

const itemClass =
    "flex h-8.5 w-full items-center gap-2.5 rounded-sm px-3 font-mono text-xs font-medium tracking-[0.5px] text-sidebar-foreground uppercase outline-none transition-colors hover:bg-secondary focus-visible:ring-2 focus-visible:ring-sidebar-ring [&>svg]:size-4 [&>svg]:shrink-0 [&>svg]:text-muted-foreground";
const activeClass =
    "data-[status=active]:bg-sidebar-accent data-[status=active]:font-bold data-[status=active]:text-sidebar-accent-foreground data-[status=active]:[&>svg]:text-sidebar-accent-foreground";

function NavLink({ item, badges }: { item: NavItem; badges: SidebarBadges }) {
    const badge = item.badge?.(badges) ?? null;
    const content = (
        <>
            <item.icon aria-hidden />
            <span>{item.label}</span>
            {badge && (
                <span
                    data-testid={`nav-badge-${item.label.toLowerCase()}`}
                    className='rounded-[3px] border border-sidebar-border bg-secondary px-1.5 py-0.5 text-3xs leading-none font-bold text-sidebar-foreground'
                >
                    {badge}
                </span>
            )}
        </>
    );
    return (
        <Link to={item.to} activeOptions={{ exact: item.exact ?? false }} className={cn(itemClass, activeClass)}>
            {content}
        </Link>
    );
}

/** Sidebar `YJkf8`: fixed 220 px, three labelled sections, and the Agent Info box. */
export function AppSidebar({ badges = {}, agent }: { badges?: SidebarBadges; agent?: AgentInfo }) {
    const [top, orchestration, system] = NAV_SECTIONS;
    const section = (group: (typeof NAV_SECTIONS)[number]) => (
        <SidebarGroup key={group.label} className='gap-0.5 p-0 px-2'>
            <SidebarGroupLabel className='h-auto rounded-none px-0.5 py-1 font-mono text-3xs font-semibold tracking-[1px] text-muted-foreground'>
                {group.label}
            </SidebarGroupLabel>
            <SidebarMenu className='gap-0.5'>
                {group.items.map((item) => (
                    <SidebarMenuItem key={item.label}>
                        <NavLink item={item} badges={badges} />
                    </SidebarMenuItem>
                ))}
            </SidebarMenu>
        </SidebarGroup>
    );
    return (
        <Sidebar collapsible='none' className='h-full w-55 shrink-0 border-r border-sidebar-border py-3'>
            <SidebarContent className='gap-0'>
                {section(top)}
                {section(orchestration)}
            </SidebarContent>
            <SidebarFooter className='gap-0.5 p-0'>
                {section(system)}
                <div className='mx-2 mt-1.5 flex flex-col gap-1 rounded-md border border-sidebar-border bg-muted px-3 py-2.5 font-mono text-3xs tracking-[0.5px]'>
                    <span className='font-bold text-sidebar-foreground'>AGENT {agent ? `v${agent.version}` : "—"}</span>
                    <AgentUptime startedAt={agent?.startedAt} />
                </div>
            </SidebarFooter>
        </Sidebar>
    );
}
