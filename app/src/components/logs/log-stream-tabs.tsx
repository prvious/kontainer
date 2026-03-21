import { Plus, ScrollText, X } from "lucide-react";

export interface LogStream {
    id: string;
    serviceId: string;
    serviceName: string;
    color: string;
}

interface LogStreamTabsProps {
    streams: LogStream[];
    activeStreamId: string | null;
    onSelect: (id: string) => void;
    onClose: (id: string) => void;
    onAdd: () => void;
}

export function LogStreamTabs({ streams, activeStreamId, onSelect, onClose, onAdd }: LogStreamTabsProps) {
    return (
        <div className='flex items-center rounded-t-lg border border-border bg-card'>
            {streams.map((stream) => {
                const isActive = stream.id === activeStreamId;
                return (
                    <button
                        key={stream.id}
                        onClick={() => onSelect(stream.id)}
                        className={`flex items-center gap-2 px-4 py-2.5 font-mono text-[11px] ${
                            isActive
                                ? "border-b-2 border-b-accent-green bg-bg-elevated font-semibold text-accent-green"
                                : "border-r border-r-border font-medium text-text-secondary hover:text-foreground"
                        }`}
                    >
                        <span className={`size-1.5 rounded-full ${stream.color}`} />
                        <ScrollText className={`size-3.5 ${isActive ? "text-accent-green" : "text-muted-foreground"}`} />
                        <span>{stream.serviceName}</span>
                        <X
                            className='size-3 text-muted-foreground hover:text-foreground'
                            onClick={(e) => {
                                e.stopPropagation();
                                onClose(stream.id);
                            }}
                        />
                    </button>
                );
            })}
            <button
                onClick={onAdd}
                className='flex size-9 items-center justify-center text-muted-foreground hover:text-foreground'
            >
                <Plus className='size-3.5' />
            </button>
        </div>
    );
}
