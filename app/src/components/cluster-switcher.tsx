import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useAuth } from "@/hooks/use-auth";
import { AddClusterDialog } from "@/components/add-cluster-dialog";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Check, ChevronDown, Cloud, LogOut, Monitor, Plus, Server, X } from "lucide-react";
import { router } from "@/router";
import type { ConnectionProfile } from "@/types/connection";

function detectProvider(url: string): { icon: typeof Cloud; label: string } {
    const lower = url.toLowerCase();
    if (lower.includes("aws") || lower.includes("amazon") || lower.includes("ec2")) return { icon: Cloud, label: "AWS" };
    if (lower.includes("gcp") || lower.includes("google") || lower.includes("gke")) return { icon: Cloud, label: "GCP" };
    if (lower.includes("azure") || lower.includes("microsoft")) return { icon: Cloud, label: "Azure" };
    if (lower.includes("digitalocean") || lower.includes("droplet")) return { icon: Cloud, label: "DO" };
    if (lower.includes("localhost") || lower.includes("127.0.0.1") || lower.includes("0.0.0.0")) return { icon: Monitor, label: "Local" };
    return { icon: Server, label: "Self-hosted" };
}

function ProfileRow({
    profile,
    isActive,
    onSwitch,
    onRemove,
    onDisconnect,
}: {
    profile: ConnectionProfile;
    isActive: boolean;
    onSwitch: () => void;
    onRemove: () => void;
    onDisconnect?: () => void;
}) {
    const provider = detectProvider(profile.agentUrl);
    const ProviderIcon = provider.icon;

    return (
        <button onClick={onSwitch} className='group flex w-full items-center gap-3 rounded-md px-2 py-2 text-left transition-colors hover:bg-accent'>
            <div className='flex size-8 shrink-0 items-center justify-center rounded-md border border-border bg-background'>
                <ProviderIcon className='size-3.5 text-muted-foreground' />
            </div>
            <div className='flex-1 overflow-hidden'>
                <div className='flex items-center gap-2'>
                    <span className='truncate text-sm font-medium'>{profile.label}</span>
                    {isActive && <Check className='size-3 shrink-0 text-primary' />}
                </div>
                <div className='flex items-center gap-1.5'>
                    <span className={`size-1.5 shrink-0 rounded-full ${isActive ? 'animate-pulse-dot bg-primary' : "bg-muted-foreground/30"}`} />
                    <span className='truncate font-mono text-[11px] text-muted-foreground'>{profile.agentUrl.replace(/^https?:\/\//, "")}</span>
                </div>
            </div>
            {isActive ? (
                <button
                    className='shrink-0 rounded p-1 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 hover:bg-muted hover:text-destructive'
                    title='Disconnect'
                    onClick={(e) => {
                        e.stopPropagation();
                        onDisconnect?.();
                    }}
                >
                    <LogOut className='size-3' />
                </button>
            ) : (
                <button
                    className='shrink-0 rounded p-1 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 hover:bg-muted hover:text-foreground'
                    onClick={(e) => {
                        e.stopPropagation();
                        onRemove();
                    }}
                >
                    <X className='size-3' />
                </button>
            )}
        </button>
    );
}

export function ClusterSwitcher() {
    const { profiles, activeProfileId, switchProfile, removeProfile, logout } = useAuth();
    const queryClient = useQueryClient();
    const [open, setOpen] = useState(false);
    const [dialogOpen, setDialogOpen] = useState(false);

    const activeProfile = profiles.find((p) => p.id === activeProfileId);
    const provider = activeProfile ? detectProvider(activeProfile.agentUrl) : null;
    const ProviderIcon = provider?.icon ?? Server;

    async function handleSwitch(profileId: string) {
        if (profileId === activeProfileId) return;
        await switchProfile(profileId);
        queryClient.clear();
        setOpen(false);
    }

    return (
        <>
            <Popover open={open} onOpenChange={setOpen}>
                <PopoverTrigger
                    render={
                        <button className='flex w-full items-center gap-3 rounded-md p-1.5 text-left transition-colors hover:bg-accent'>
                            <div className='flex size-8 shrink-0 items-center justify-center rounded-md border border-border bg-background'>
                                <ProviderIcon className='size-3.5 text-muted-foreground' />
                            </div>
                            <div className='flex-1 overflow-hidden'>
                                <p className='truncate text-sm font-semibold'>{activeProfile?.label ?? "No connection"}</p>
                                <div className='flex items-center gap-1.5'>
                                    <span className='animate-pulse-dot size-1.5 shrink-0 rounded-full bg-primary' />
                                    <span className='truncate text-[11px] text-muted-foreground'>
                                        {activeProfile ? activeProfile.agentUrl.replace(/^https?:\/\//, "") : "Connect a cluster"}
                                    </span>
                                </div>
                            </div>
                            <ChevronDown className={`size-4 shrink-0 text-muted-foreground transition-transform ${open ? "" : "rotate-180"}`} />
                        </button>
                    }
                />

                <PopoverContent side='top' align='start' className='w-[340px]'>
                    <div className='flex items-center justify-between px-2'>
                        <div className='flex items-center gap-2'>
                            <span className='text-sm font-medium'>Clusters</span>
                            <span className='rounded-full bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground'>{profiles.length}</span>
                        </div>
                        <button
                            onClick={() => {
                                setOpen(false);
                                setDialogOpen(true);
                            }}
                            className='flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium text-primary transition-colors hover:bg-primary/10'
                        >
                            <Plus className='size-3' />
                            Add
                        </button>
                    </div>

                    <div className='max-h-64 overflow-y-auto'>
                        {profiles.length === 0 ? (
                            <div className='px-2 py-4 text-center text-xs text-muted-foreground'>No clusters connected</div>
                        ) : (
                            <div className='space-y-0.5'>
                                {profiles.map((profile) => (
                                    <ProfileRow
                                        key={profile.id}
                                        profile={profile}
                                        isActive={profile.id === activeProfileId}
                                        onSwitch={() => handleSwitch(profile.id)}
                                        onRemove={() => removeProfile(profile.id)}
                                        onDisconnect={() => {
                                            setOpen(false);
                                            void logout().finally(() => {
                                                queryClient.clear();
                                                router.invalidate().finally(() => router.navigate({ to: "/login", search: { redirect: undefined } }));
                                            });
                                        }}
                                    />
                                ))}
                            </div>
                        )}
                    </div>
                </PopoverContent>
            </Popover>

            <AddClusterDialog open={dialogOpen} onOpenChange={setDialogOpen} />
        </>
    );
}
