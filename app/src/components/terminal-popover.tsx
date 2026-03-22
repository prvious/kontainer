import { useState } from "react";
import { useMatches, useNavigate } from "@tanstack/react-router";
import { Terminal, X } from "lucide-react";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { useTerminal } from "@/hooks/use-terminal";

const DOT_COLORS = ["bg-accent-green", "bg-cyan-400", "bg-orange-400", "bg-purple-400", "bg-red-400"] as const;

export function TerminalPopover() {
    const [open, setOpen] = useState(false);
    const navigate = useNavigate();
    const matches = useMatches();
    const { sessions, switchSession, closeSession, setShowPanel, setMinimized } = useTerminal();

    const isOnExecRoute = matches[matches.length - 1]?.fullPath?.includes("/exec") ?? false;

    function handleSessionClick(id: string) {
        switchSession(id);
        if (!isOnExecRoute) {
            setShowPanel(true);
            setMinimized(false);
        }
        setOpen(false);
    }

    function handleNewSession() {
        if (isOnExecRoute) return;
        setOpen(false);
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        void navigate({ to: "/exec" as any });
    }

    const trigger = (
        <button className='flex h-full items-center gap-[5px] px-2.5 text-text-secondary transition-colors hover:text-foreground'>
            <Terminal className='size-[11px] text-accent-green' />
            <span className='font-mono text-[10px] font-medium'>
                {sessions.length} TERMINALS
            </span>
        </button>
    );

    return (
        <Popover open={open} onOpenChange={setOpen}>
            <PopoverTrigger render={trigger} />

            <PopoverContent side='top' align='end' className='w-[260px] border-border bg-card p-2'>
                {/* Header */}
                <div className='mb-2 flex items-center justify-between px-2 py-1'>
                    <span className='font-mono text-[9px] font-medium tracking-widest text-muted-foreground uppercase'>
                        Terminal Sessions
                    </span>
                    <span className='rounded bg-bg-elevated px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground'>
                        {sessions.length}
                    </span>
                </div>

                {/* Session list */}
                <div className='max-h-56 overflow-y-auto'>
                    {sessions.length === 0 ? (
                        <div className='px-2 py-4 text-center font-mono text-[11px] text-muted-foreground'>
                            No terminal sessions
                        </div>
                    ) : (
                        <div className='space-y-0.5'>
                            {sessions.map((session, index) => {
                                const name = session.target.containerName || session.target.containerId.slice(0, 12);
                                return (
                                    <button
                                        key={session.id}
                                        onClick={() => handleSessionClick(session.id)}
                                        className='group flex w-full items-center gap-3 rounded px-2 py-2 text-left transition-colors hover:bg-bg-elevated'
                                    >
                                        <span className={`size-2 shrink-0 rounded-full ${DOT_COLORS[index % DOT_COLORS.length]}`} />
                                        <span className='flex-1 truncate font-mono text-[11px] text-foreground'>
                                            {name}
                                        </span>
                                        <span
                                            className='shrink-0 rounded p-0.5 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 hover:text-destructive'
                                            role='button'
                                            tabIndex={0}
                                            title='Close session'
                                            onClick={(e) => {
                                                e.stopPropagation();
                                                closeSession(session.id);
                                            }}
                                        >
                                            <X className='size-3' />
                                        </span>
                                    </button>
                                );
                            })}
                        </div>
                    )}
                </div>

                {/* Footer */}
                <div className='mt-2 border-t border-border pt-2'>
                    <button
                        onClick={handleNewSession}
                        disabled={isOnExecRoute}
                        className='flex w-full items-center gap-2 rounded px-2 py-1.5 font-mono text-[11px] font-medium text-accent-green transition-colors hover:bg-accent-green/10 disabled:pointer-events-none disabled:opacity-40'
                    >
                        <Terminal className='size-3' />
                        Open Exec
                    </button>
                </div>
            </PopoverContent>
        </Popover>
    );
}
