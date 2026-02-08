import { Link, useMatches } from "@tanstack/react-router";
import {
    Sidebar,
    SidebarContent,
    SidebarFooter,
    SidebarGroup,
    SidebarGroupContent,
    SidebarGroupLabel,
    SidebarHeader,
    SidebarMenu,
    SidebarMenuButton,
    SidebarMenuItem,
    SidebarSeparator,
} from "@/components/ui/sidebar";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { useAuth } from "@/hooks/use-auth";
import { Logo } from "@/components/logo";
import { ClusterSwitcher } from "@/components/cluster-switcher";
import { Activity, ChevronsUpDown, Container, HardDrive, KeyRound, Layers, LayoutDashboard, LogOut, Moon, Network, Server, Sun } from "lucide-react";
import { useAppearance } from "@/hooks/use-appearance";
import { router } from "@/router";

const navigation = [
    { label: "Overview", to: "/" as const, icon: LayoutDashboard },
    { label: "Services", to: "/services" as const, icon: Container },
    { label: "Stacks", to: "/stacks" as const, icon: Layers },
    { label: "Nodes", to: "/nodes" as const, icon: Server },
    { label: "Networks", to: "/networks" as const, icon: Network },
    { label: "Volumes", to: "/volumes" as const, icon: HardDrive },
    { label: "Secrets & Configs", to: "/secrets-configs" as const, icon: KeyRound },
    { label: "Tasks", to: "/tasks" as const, icon: Activity },
];

function getInitials(name: string): string {
    return name
        .split(" ")
        .map((n) => n[0])
        .join("")
        .toUpperCase()
        .slice(0, 2);
}

export function AppSidebar() {
    const { user, logout } = useAuth();
    const { resolvedScheme, setScheme } = useAppearance();
    const matches = useMatches();

    const currentPath = matches[matches.length - 1]?.fullPath ?? "/";

    function isActive(to: string): boolean {
        if (to === "/") return currentPath === "/";
        return currentPath.startsWith(to);
    }

    return (
        <Sidebar collapsible='icon'>
            <SidebarHeader className='p-4'>
                <Link to='/'>
                    <Logo />
                </Link>
            </SidebarHeader>

            <SidebarSeparator />

            <SidebarContent>
                <SidebarGroup>
                    <SidebarGroupLabel>Navigation</SidebarGroupLabel>
                    <SidebarGroupContent>
                        <SidebarMenu>
                            {navigation.map((item) => (
                                <SidebarMenuItem key={item.to}>
                                    <Link to={item.to}>
                                        <SidebarMenuButton isActive={isActive(item.to)} tooltip={item.label}>
                                            <item.icon className='size-4' />
                                            <span>{item.label}</span>
                                        </SidebarMenuButton>
                                    </Link>
                                </SidebarMenuItem>
                            ))}
                        </SidebarMenu>
                    </SidebarGroupContent>
                </SidebarGroup>

                <ClusterSwitcher />
            </SidebarContent>

            <SidebarFooter>
                <SidebarMenu>
                    <SidebarMenuItem>
                        <DropdownMenu>
                            <DropdownMenuTrigger
                                render={(props) => (
                                    <SidebarMenuButton
                                        {...props}
                                        size='lg'
                                        className='data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground'
                                    />
                                )}
                            >
                                <Avatar className='size-7'>
                                    <AvatarImage src={user?.avatarUrl} alt={user?.name} />
                                    <AvatarFallback className='text-[10px]'>{user ? getInitials(user.name) : "?"}</AvatarFallback>
                                </Avatar>
                                <div className='grid flex-1 text-left text-sm leading-tight'>
                                    <span className='truncate font-medium'>{user?.name}</span>
                                    <span className='truncate text-xs text-muted-foreground'>{user?.email}</span>
                                </div>
                                <ChevronsUpDown className='ml-auto size-4' />
                            </DropdownMenuTrigger>
                            <DropdownMenuContent side='top' align='start' className='w-56'>
                                <DropdownMenuItem onClick={() => setScheme(resolvedScheme === "dark" ? "light" : "dark")} closeOnClick={false}>
                                    {resolvedScheme === "dark" ? <Sun className='size-4' /> : <Moon className='size-4' />}
                                    {resolvedScheme === "dark" ? "Light mode" : "Dark mode"}
                                </DropdownMenuItem>
                                <DropdownMenuSeparator />
                                <DropdownMenuItem
                                    onClick={() => {
                                        void logout().then(() => router.navigate({ to: "/login", search: { redirect: undefined } }));
                                    }}
                                >
                                    <LogOut className='size-4' />
                                    Disconnect
                                </DropdownMenuItem>
                            </DropdownMenuContent>
                        </DropdownMenu>
                    </SidebarMenuItem>
                </SidebarMenu>
            </SidebarFooter>
        </Sidebar>
    );
}
