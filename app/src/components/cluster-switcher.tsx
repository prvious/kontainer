import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuGroup,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { SidebarGroup, SidebarGroupContent, SidebarGroupLabel, SidebarMenu, SidebarMenuButton, SidebarMenuItem } from "@/components/ui/sidebar";
import { useAuth } from "@/hooks/use-auth";
import { AddClusterDialog } from "@/components/add-cluster-dialog";
import { ChevronDown, Plus, X } from "lucide-react";

export function ClusterSwitcher() {
    const { profiles, activeProfileId, agentUrl, switchProfile, removeProfile } = useAuth();
    const queryClient = useQueryClient();
    const [dialogOpen, setDialogOpen] = useState(false);

    const activeProfile = profiles.find((p) => p.id === activeProfileId);

    async function handleSwitch(profileId: string) {
        if (profileId === activeProfileId) return;
        await switchProfile(profileId);
        queryClient.clear();
    }

    return (
        <>
            <SidebarGroup className='mt-auto'>
                <SidebarGroupLabel>Connection</SidebarGroupLabel>
                <SidebarGroupContent>
                    <SidebarMenu>
                        <SidebarMenuItem>
                            <DropdownMenu>
                                <DropdownMenuTrigger
                                    render={(props) => (
                                        <SidebarMenuButton
                                            {...props}
                                            className='data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground'
                                        >
                                            <span className='animate-pulse-dot size-1.5 shrink-0 rounded-full bg-success' />
                                            <span className='flex-1 truncate text-sm'>{activeProfile?.label ?? "No connection"}</span>
                                            <ChevronDown className='ml-auto size-4 opacity-50' />
                                        </SidebarMenuButton>
                                    )}
                                />

                                <DropdownMenuContent side='top' align='start' className='w-64'>
                                    <DropdownMenuGroup>
                                        <DropdownMenuLabel>Clusters</DropdownMenuLabel>
                                        {profiles.map((profile) => (
                                            <DropdownMenuItem key={profile.id} onClick={() => handleSwitch(profile.id)} className='gap-2'>
                                                <span
                                                    className={`size-1.5 shrink-0 rounded-full ${profile.id === activeProfileId ? "bg-success" : "bg-muted-foreground/30"}`}
                                                />
                                                <div className='flex flex-1 flex-col overflow-hidden'>
                                                    <span className='truncate text-sm font-medium'>{profile.label}</span>
                                                    <span className='truncate font-mono text-xs text-muted-foreground'>
                                                        {profile.agentUrl.replace(/^https?:\/\//, "")}
                                                    </span>
                                                </div>
                                                {profile.id !== activeProfileId && (
                                                    <button
                                                        className='ml-auto shrink-0 rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground'
                                                        onClick={(e) => {
                                                            e.stopPropagation();
                                                            removeProfile(profile.id);
                                                        }}
                                                    >
                                                        <X className='size-3' />
                                                    </button>
                                                )}
                                            </DropdownMenuItem>
                                        ))}
                                    </DropdownMenuGroup>

                                    {profiles.length === 0 && !agentUrl && (
                                        <div className='px-1.5 py-2 text-center text-xs text-muted-foreground'>No clusters connected</div>
                                    )}

                                    <DropdownMenuSeparator />
                                    <DropdownMenuItem onClick={() => setDialogOpen(true)}>
                                        <Plus className='size-4' />
                                        Add cluster
                                    </DropdownMenuItem>
                                </DropdownMenuContent>
                            </DropdownMenu>
                        </SidebarMenuItem>
                    </SidebarMenu>
                </SidebarGroupContent>
            </SidebarGroup>

            <AddClusterDialog open={dialogOpen} onOpenChange={setDialogOpen} />
        </>
    );
}
