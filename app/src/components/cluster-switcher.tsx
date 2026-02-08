import { useState, useRef, useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useAuth } from "@/hooks/use-auth";
import { AddClusterDialog } from "@/components/add-cluster-dialog";
import { Check, ChevronDown, Cloud, Monitor, Plus, Server, X } from "lucide-react";
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

function ProfileRow({ profile, isActive, onSwitch, onRemove }: { profile: ConnectionProfile; isActive: boolean; onSwitch: () => void; onRemove: () => void }) {
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
            {!isActive && (
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
    const { profiles, activeProfileId, switchProfile, removeProfile } = useAuth();
    const queryClient = useQueryClient();
    const [open, setOpen] = useState(false);
    const [dialogOpen, setDialogOpen] = useState(false);
    const containerRef = useRef<HTMLDivElement>(null);

    const activeProfile = profiles.find((p) => p.id === activeProfileId);
    const provider = activeProfile ? detectProvider(activeProfile.agentUrl) : null;
    const ProviderIcon = provider?.icon ?? Server;

    useEffect(() => {
        function handleClickOutside(e: MouseEvent) {
            if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
                setOpen(false);
            }
        }
        if (open) {
            document.addEventListener("mousedown", handleClickOutside);
            return () => document.removeEventListener("mousedown", handleClickOutside);
        }
    }, [open]);

    async function handleSwitch(profileId: string) {
        if (profileId === activeProfileId) return;
        await switchProfile(profileId);
        queryClient.clear();
        setOpen(false);
    }

    return (
        <>
            <div ref={containerRef} className='relative'>
                {/* Trigger Button */}
                <button onClick={() => setOpen(!open)} className='flex w-full items-center gap-3 rounded-md p-1.5 text-left transition-colors hover:bg-accent'>
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
                    <ChevronDown className={`size-4 shrink-0 text-muted-foreground transition-transform ${open ? "rotate-180" : ""}`} />
                </button>

                {/* Dropdown Panel */}
                {open && (
                    <div className='absolute top-full left-0 z-50 mt-1 w-[340px] rounded-lg border border-border bg-popover p-2 shadow-lg'>
                        <div className='mb-2 flex items-center justify-between px-2'>
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
                                        />
                                    ))}
                                </div>
                            )}
                        </div>
                    </div>
                )}
            </div>

            <AddClusterDialog open={dialogOpen} onOpenChange={setDialogOpen} />
        </>
    );
}
