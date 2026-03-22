import { Plus, Terminal, X } from "lucide-react";
import { useTerminal } from "@/hooks/use-terminal";

const TAB_COLORS = ["bg-accent-green", "bg-accent-cyan", "bg-accent-orange", "bg-accent-purple", "bg-accent-red"];

interface ExecSessionTabsProps {
    onAdd: () => void;
}

export function ExecSessionTabs({ onAdd }: ExecSessionTabsProps) {
    const { sessions, activeSessionId, switchSession, closeSession } = useTerminal();

    return (
        <div className='flex items-center overflow-x-auto rounded-t-lg border border-border bg-card'>
            {sessions.map((session, i) => {
                const isActive = session.id === activeSessionId;
                const name = session.target.containerName || session.target.containerId.slice(0, 12);
                return (
                    <button
                        key={session.id}
                        onClick={() => switchSession(session.id)}
                        className={`flex shrink-0 items-center gap-2 px-4 py-2.5 font-mono text-[11px] ${
                            isActive
                                ? 'border-b-2 border-b-accent-green bg-bg-elevated font-semibold text-accent-green'
                                : 'border-r border-r-border font-medium text-text-secondary hover:text-foreground'
                        }`}
                    >
                        <span className={`size-1.5 rounded-full ${TAB_COLORS[i % TAB_COLORS.length]}`} />
                        <Terminal className={`size-3.5 ${isActive ? "text-accent-green" : "text-muted-foreground"}`} />
                        <span>{name}</span>
                        <X
                            className='size-3 text-muted-foreground hover:text-foreground'
                            onClick={(e) => {
                                e.stopPropagation();
                                closeSession(session.id);
                            }}
                        />
                    </button>
                );
            })}
            <button onClick={onAdd} className='flex size-9 shrink-0 items-center justify-center text-muted-foreground hover:text-foreground'>
                <Plus className='size-3.5' />
            </button>
        </div>
    );
}
