import { useTerminal } from "@/hooks/use-terminal";

export function ExecConnectionBar() {
    const { sessions, activeSessionId } = useTerminal();
    const activeSession = sessions.find((s) => s.id === activeSessionId);
    const name = activeSession?.target.containerName || activeSession?.target.containerId.slice(0, 12) || "--";

    return (
        <div className='flex items-center gap-2 rounded-md border border-border bg-bg-elevated px-3 py-2'>
            <span className='size-1.5 rounded-full bg-accent-green' />
            <span className='font-mono text-[11px] text-text-secondary'>Connected to {name}</span>
            <span className='font-mono text-[11px] text-muted-foreground'>
                {sessions.length} active session{sessions.length !== 1 ? "s" : ""} | PID: -- | TTY: pts/0
            </span>
        </div>
    );
}
