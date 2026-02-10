import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { useAuth } from "@/hooks/use-auth";
import { Loader2 } from "lucide-react";

interface AddClusterDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
}

export function AddClusterDialog({ open, onOpenChange }: AddClusterDialogProps) {
    const { login } = useAuth();
    const [label, setLabel] = useState("");
    const [url, setUrl] = useState("");
    const [error, setError] = useState<string | null>(null);
    const [isConnecting, setIsConnecting] = useState(false);

    async function handleConnect() {
        setError(null);

        const trimmedUrl = url.trim().replace(/\/+$/, "");
        const trimmedLabel = label.trim();

        if (!trimmedLabel) {
            setError("Label is required");
            return;
        }

        if (!trimmedUrl) {
            setError("Agent URL is required");
            return;
        }

        setIsConnecting(true);

        try {
            await invoke("ping_agent", { url: trimmedUrl });
        } catch {
            setError("Could not reach agent at this URL");
            setIsConnecting(false);
            return;
        }

        try {
            await login(trimmedUrl, trimmedLabel);
            setLabel("");
            setUrl("");
            setError(null);
            onOpenChange(false);
        } catch (err) {
            setError(err instanceof Error ? err.message : "Authentication failed");
        } finally {
            setIsConnecting(false);
        }
    }

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>Add cluster</DialogTitle>
                    <DialogDescription>Connect to a new Kontainer agent. You'll be redirected to authenticate.</DialogDescription>
                </DialogHeader>

                <div className='grid gap-3'>
                    <div className='grid gap-1.5'>
                        <Label htmlFor='cluster-label'>Label</Label>
                        <Input
                            id='cluster-label'
                            placeholder='e.g. Production'
                            value={label}
                            onChange={(e) => setLabel(e.target.value)}
                            disabled={isConnecting}
                        />
                    </div>
                    <div className='grid gap-1.5'>
                        <Label htmlFor='cluster-url'>Agent URL</Label>
                        <Input
                            id='cluster-url'
                            placeholder='https://agent.example.com'
                            value={url}
                            onChange={(e) => setUrl(e.target.value)}
                            disabled={isConnecting}
                        />
                    </div>
                    {error && <p className='text-sm text-destructive'>{error}</p>}
                </div>

                <DialogFooter>
                    <Button variant='outline' onClick={() => onOpenChange(false)} disabled={isConnecting}>
                        Cancel
                    </Button>
                    <Button onClick={handleConnect} disabled={isConnecting}>
                        {isConnecting && <Loader2 className='animate-spin' />}
                        Connect
                    </Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}
