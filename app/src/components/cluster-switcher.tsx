import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useAuth } from "@/hooks/use-auth";
import { AddClusterDialog } from "@/components/add-cluster-dialog";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ChevronDown, Plus } from "lucide-react";
import { router } from "@/router";
import type { ConnectionProfile } from "@/types/connection";


const DOT_COLORS = [
    "bg-accent-green",
    "bg-cyan-400",
    "bg-orange-400",
    "bg-purple-400",
    "bg-red-400",
] as const;

function ProfileRow({
    profile,
    isActive,
    dotColor,
    onSwitch,
    onRemove,
    onDisconnect,
}: {
    profile: ConnectionProfile;
    isActive: boolean;
    dotColor: string;
    onSwitch: () => void;
    onRemove: () => void;
    onDisconnect?: () => void;
}) {
    return (
        <button
            onClick={onSwitch}
            className='group flex w-full items-center gap-3 rounded px-2 py-2 text-left transition-colors hover:bg-bg-elevated'
        >
            <span className={`size-2 shrink-0 rounded-full ${dotColor}`} />
            <div className='flex-1 overflow-hidden'>
                <div className='flex items-center gap-2'>
                    <span className='truncate text-sm font-bold text-foreground'>{profile.label}</span>
                </div>
                <span className='truncate font-mono text-[10px] text-muted-foreground'>
                    {profile.agentUrl.replace(/^https?:\/\//, "")}
                </span>
            </div>
            {isActive ? (
                <button
                    className='shrink-0 rounded px-1.5 py-0.5 font-mono text-[10px] font-medium text-accent-green transition-colors hover:bg-accent-green/10'
                    title='Disconnect'
                    onClick={(e) => {
                        e.stopPropagation();
                        onDisconnect?.();
                    }}
                >
                    Active
                </button>
            ) : (
                <ChevronDown className='size-3 shrink-0 -rotate-90 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100' />
            )}
            {!isActive && (
                <button
                    className='shrink-0 rounded p-0.5 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 hover:text-destructive'
                    title='Remove'
                    onClick={(e) => {
                        e.stopPropagation();
                        onRemove();
                    }}
                >
                    <span className='font-mono text-[10px]'>✕</span>
                </button>
            )}
        </button>
    );
}

export function ClusterSwitcher({ variant = "sidebar" }: { variant?: "sidebar" | "titlebar" }) {
    const { profiles, activeProfileId, switchProfile, removeProfile, logout } = useAuth();
    const queryClient = useQueryClient();
    const [open, setOpen] = useState(false);
    const [dialogOpen, setDialogOpen] = useState(false);

    const activeProfile = profiles.find((p) => p.id === activeProfileId);

    async function handleSwitch(profileId: string) {
        if (profileId === activeProfileId) return;
        await switchProfile(profileId);
        queryClient.clear();
        setOpen(false);
    }

    const trigger =
        variant === "titlebar" ? (
            <button className='flex items-center gap-2 text-text-secondary transition-colors hover:text-foreground'>
                <span className='animate-pulse-dot size-[7px] shrink-0 rounded-full bg-accent-green' />
                <span className='font-mono text-[11px] font-medium tracking-[0.5px]'>
                    {activeProfile?.label ?? "No cluster"}
                </span>
                <ChevronDown className='size-3 text-muted-foreground' />
            </button>
        ) : (
            <button className='flex w-full items-center gap-3 rounded-md p-1.5 text-left transition-colors hover:bg-accent'>
                <span className='animate-pulse-dot size-2 shrink-0 rounded-full bg-accent-green' />
                <div className='flex-1 overflow-hidden'>
                    <p className='truncate text-sm font-semibold'>{activeProfile?.label ?? "No connection"}</p>
                    <span className='truncate font-mono text-[11px] text-muted-foreground'>
                        {activeProfile ? activeProfile.agentUrl.replace(/^https?:\/\//, "") : "Connect a cluster"}
                    </span>
                </div>
                <ChevronDown className={`size-4 shrink-0 text-muted-foreground transition-transform ${open ? "rotate-180" : ""}`} />
            </button>
        );

    return (
        <>
            <Popover open={open} onOpenChange={setOpen}>
                <PopoverTrigger render={trigger} />

                <PopoverContent
                    side={variant === "titlebar" ? "bottom" : "top"}
                    align='center'
                    className='w-[300px] border-border bg-card p-2'
                >
                    {/* Header */}
                    <div className='mb-2 flex items-center justify-between px-2 py-1'>
                        <span className='font-mono text-[9px] font-medium uppercase tracking-widest text-muted-foreground'>
                            Switch Cluster
                        </span>
                        <span className='rounded bg-bg-elevated px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground'>
                            {profiles.length}
                        </span>
                    </div>

                    {/* Profile list */}
                    <div className='max-h-64 overflow-y-auto'>
                        {profiles.length === 0 ? (
                            <div className='px-2 py-4 text-center font-mono text-[11px] text-muted-foreground'>
                                No clusters connected
                            </div>
                        ) : (
                            <div className='space-y-0.5'>
                                {profiles.map((profile, index) => (
                                    <ProfileRow
                                        key={profile.id}
                                        profile={profile}
                                        isActive={profile.id === activeProfileId}
                                        dotColor={DOT_COLORS[index % DOT_COLORS.length]}
                                        onSwitch={() => handleSwitch(profile.id)}
                                        onRemove={() => removeProfile(profile.id)}
                                        onDisconnect={() => {
                                            setOpen(false);
                                            void logout().finally(() => {
                                                queryClient.clear();
                                                router
                                                    .invalidate()
                                                    .finally(() =>
                                                        router.navigate({ to: "/login", search: { redirect: undefined } }),
                                                    );
                                            });
                                        }}
                                    />
                                ))}
                            </div>
                        )}
                    </div>

                    {/* Footer: Add New Agent */}
                    <div className='mt-2 border-t border-border pt-2'>
                        <button
                            onClick={() => {
                                setOpen(false);
                                setDialogOpen(true);
                            }}
                            className='flex w-full items-center gap-2 rounded px-2 py-1.5 font-mono text-[11px] font-medium text-cyan-400 transition-colors hover:bg-cyan-400/10'
                        >
                            <Plus className='size-3' />
                            Add New Agent
                        </button>
                    </div>
                </PopoverContent>
            </Popover>

            <AddClusterDialog open={dialogOpen} onOpenChange={setDialogOpen} />
        </>
    );
}
