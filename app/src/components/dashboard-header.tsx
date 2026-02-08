import { useMatches } from "@tanstack/react-router";
import { Bell, ChevronRight, Moon, Plus, Search, Sun } from "lucide-react";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { useAuth } from "@/hooks/use-auth";
import { useAppearance } from "@/hooks/use-appearance";
import { router } from "@/router";
import { LogOut } from "lucide-react";

function getInitials(name: string): string {
    return name
        .split(" ")
        .map((n) => n[0])
        .join("")
        .toUpperCase()
        .slice(0, 2);
}

const routeLabels: Record<string, string> = {
    "/": "Overview",
    "/services": "Services",
    "/tasks": "Containers",
    "/nodes": "Nodes",
    "/stacks": "Stacks",
    "/networks": "Networking",
    "/volumes": "Volumes",
    "/logs": "Logs",
    "/settings": "Settings",
    "/secrets-configs": "Secrets & Configs",
};

export function DashboardHeader() {
    const { user, logout, profiles, activeProfileId } = useAuth();
    const { resolvedScheme, setScheme } = useAppearance();
    const matches = useMatches();
    const activeProfile = profiles.find((p) => p.id === activeProfileId);

    const currentPath = matches[matches.length - 1]?.fullPath ?? "/";
    // Handle dynamic routes like /services/$serviceId
    const basePath = Object.keys(routeLabels).find((k) => currentPath === k || (k !== "/" && currentPath.startsWith(k))) ?? "/";
    const currentLabel = routeLabels[basePath] ?? "Overview";

    return (
        <header className='flex h-14 shrink-0 items-center gap-4 border-b border-border bg-card px-6'>
            {/* Breadcrumb */}
            <div className='flex items-center gap-1.5 text-sm'>
                <span className='text-muted-foreground'>{activeProfile?.label ?? "Cluster"}</span>
                <ChevronRight className='size-3 text-muted-foreground/50' />
                <span className='font-medium'>{currentLabel}</span>
            </div>

            {/* Spacer */}
            <div className='flex-1' />

            {/* Search */}
            <div className='relative'>
                <Search className='absolute top-1/2 left-3 size-3.5 -translate-y-1/2 text-muted-foreground' />
                <Input placeholder='Search...' className='h-8 w-56 rounded-md bg-background pl-9 text-sm' readOnly />
                <kbd className='absolute top-1/2 right-2 -translate-y-1/2 rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground'>
                    ⌘K
                </kbd>
            </div>

            {/* Notification Bell */}
            <Button variant='ghost' size='sm' className='relative size-8 p-0'>
                <Bell className='size-4' />
                <span className='absolute top-1 right-1.5 size-1.5 rounded-full bg-primary' />
            </Button>

            {/* New Service Button */}
            <Button size='sm' className='h-8 gap-1.5'>
                <Plus className='size-3.5' />
                New Service
            </Button>

            {/* User Avatar with Dropdown */}
            <DropdownMenu>
                <DropdownMenuTrigger
                    render={(props) => (
                        <button
                            {...props}
                            className='rounded-full ring-offset-background focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none'
                        >
                            <Avatar className='size-8'>
                                <AvatarImage src={user?.avatarUrl} alt={user?.name} />
                                <AvatarFallback className='bg-primary/10 text-[11px] font-medium text-primary'>
                                    {user ? getInitials(user.name) : "?"}
                                </AvatarFallback>
                            </Avatar>
                        </button>
                    )}
                />
                <DropdownMenuContent align='end' className='w-48'>
                    <div className='px-2 py-1.5'>
                        <p className='text-sm font-medium'>{user?.name}</p>
                        <p className='text-xs text-muted-foreground'>{user?.email}</p>
                    </div>
                    <DropdownMenuSeparator />
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
        </header>
    );
}
