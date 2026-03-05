import { useEffect, useState } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { useAuth } from "@/hooks/use-auth";

export function ConnectionSection() {
    const { profiles, activeProfileId, updateProfile } = useAuth();
    const activeProfile = profiles.find((p) => p.id === activeProfileId);

    const [label, setLabel] = useState(activeProfile?.label ?? "");
    const [agentUrl, setAgentUrl] = useState(activeProfile?.agentUrl ?? "");
    const [saving, setSaving] = useState(false);

    useEffect(() => {
        setLabel(activeProfile?.label ?? "");
        setAgentUrl(activeProfile?.agentUrl ?? "");
    }, [activeProfile?.id, activeProfile?.label, activeProfile?.agentUrl]);

    if (!activeProfile) {
        return (
            <section>
                <h2 className='mb-3 border-b border-border pb-2 font-mono text-xs tracking-widest text-muted-foreground uppercase'>Connection</h2>
                <p className='py-3 text-sm text-muted-foreground'>No active connection. Connect to a cluster first.</p>
            </section>
        );
    }

    const hasChanges = label !== activeProfile.label || agentUrl !== activeProfile.agentUrl;

    async function handleSave() {
        if (!activeProfile || !hasChanges) return;
        setSaving(true);
        try {
            await updateProfile(activeProfile.id, { label, agentUrl });
        } catch (err) {
            console.error("Failed to update profile:", err);
        } finally {
            setSaving(false);
        }
    }

    return (
        <section>
            <h2 className='mb-3 border-b border-border pb-2 font-mono text-xs tracking-widest text-muted-foreground uppercase'>Connection</h2>
            <div className='divide-y divide-border/50'>
                <div className='flex items-center justify-between py-3'>
                    <div className='space-y-0.5'>
                        <Label htmlFor='profile-label'>Display name</Label>
                        <p className='text-xs text-muted-foreground'>A friendly name for this cluster</p>
                    </div>
                    <Input id='profile-label' className='w-64' value={label} placeholder='e.g. Production' onChange={(e) => setLabel(e.target.value)} />
                </div>

                <div className='flex items-center justify-between py-3'>
                    <div className='space-y-0.5'>
                        <Label htmlFor='agent-url'>Agent URL</Label>
                        <p className='text-xs text-muted-foreground'>Endpoint for this cluster&apos;s agent</p>
                    </div>
                    <Input
                        id='agent-url'
                        className='w-64'
                        value={agentUrl}
                        placeholder='https://agent.example.com'
                        onChange={(e) => setAgentUrl(e.target.value)}
                    />
                </div>
            </div>

            <div className='flex justify-end pt-3'>
                <Button size='sm' disabled={!hasChanges || saving} onClick={handleSave}>
                    {saving ? "Saving..." : "Save"}
                </Button>
            </div>
        </section>
    );
}
