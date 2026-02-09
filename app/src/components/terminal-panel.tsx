import { useCallback, useRef, useState } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { TerminalSessionView } from "@/components/terminal-session-view";
import { Button } from "@/components/ui/button";
import { ChevronDown, ChevronUp, Maximize2, Minimize2, X, XCircle } from "lucide-react";
import { useTerminal } from "@/hooks/use-terminal";
import type { ExecStatus } from "@/hooks/use-exec-stream";

const MIN_HEIGHT = 150;
const DEFAULT_HEIGHT = 300;
const MAX_HEIGHT_VH = 0.8;

const statusDotColors: Record<string, string> = {
    disconnected: "bg-zinc-500",
    connecting: "bg-yellow-500 animate-pulse",
    connected: "bg-emerald-500",
    error: "bg-red-500",
    exited: "bg-zinc-500",
};

export function TerminalPanel() {
    const { sessions, activeSessionId, switchSession, closeSession, closeAllSessions } = useTerminal();
    const [panelHeight, setPanelHeight] = useState(DEFAULT_HEIGHT);
    const [isFullscreen, setIsFullscreen] = useState(false);
    const [isMinimized, setIsMinimized] = useState(false);
    const isDraggingRef = useRef(false);
    const [sessionStatuses, setSessionStatuses] = useState<Record<string, ExecStatus>>({});

    const handleStatusChange = useCallback((sessionId: string, status: ExecStatus) => {
        setSessionStatuses((prev) => {
            if (prev[sessionId] === status) return prev;
            return { ...prev, [sessionId]: status };
        });
    }, []);

    const handleMouseDown = useCallback(
        (e: React.MouseEvent) => {
            e.preventDefault();
            isDraggingRef.current = true;
            const startY = e.clientY;
            const startHeight = panelHeight;
            const maxH = window.innerHeight * MAX_HEIGHT_VH;

            const onMouseMove = (ev: MouseEvent) => {
                if (!isDraggingRef.current) return;
                const delta = startY - ev.clientY;
                const newHeight = Math.max(MIN_HEIGHT, Math.min(maxH, startHeight + delta));
                setPanelHeight(newHeight);
            };

            const onMouseUp = () => {
                isDraggingRef.current = false;
                document.removeEventListener("mousemove", onMouseMove);
                document.removeEventListener("mouseup", onMouseUp);
            };

            document.addEventListener("mousemove", onMouseMove);
            document.addEventListener("mouseup", onMouseUp);
        },
        [panelHeight],
    );

    const panelClasses = isFullscreen
        ? "fixed inset-0 z-50 flex flex-col bg-[#1a1b26]"
        : "fixed bottom-0 left-0 right-0 z-40 flex flex-col border-t border-border bg-[#1a1b26] shadow-[0_-4px_20px_rgba(0,0,0,0.3)]";

    const panelStyle = isFullscreen || isMinimized ? undefined : { height: panelHeight };

    return (
        <div className={panelClasses} style={panelStyle}>
            {/* Resize handle */}
            {!isFullscreen && !isMinimized && (
                <div className='h-1 cursor-row-resize bg-border transition-colors hover:bg-primary/50' onMouseDown={handleMouseDown} />
            )}

            <Tabs value={activeSessionId ?? undefined} onValueChange={switchSession} className='flex min-h-0 flex-1 flex-col overflow-hidden'>
                {/* Tab bar */}
                <div className='flex items-center border-b border-white/10'>
                    <TabsList variant='line' className='h-8 gap-0 overflow-x-auto bg-transparent px-1'>
                        {sessions.map((session) => {
                            const name = session.target.containerName || session.target.containerId.slice(0, 12);
                            const sessionStatus = sessionStatuses[session.id] ?? "disconnected";
                            return (
                                <TabsTrigger
                                    key={session.id}
                                    value={session.id}
                                    className='group/tab h-7 max-w-[180px] gap-1.5 rounded-none border-none px-2 text-xs text-white/60 data-active:bg-white/5 data-active:text-white'
                                >
                                    <span className={`size-1.5 shrink-0 rounded-full ${statusDotColors[sessionStatus]}`} />
                                    <span className='truncate'>{name}</span>
                                    <span
                                        className='ml-0.5 shrink-0 rounded p-0.5 opacity-0 transition-opacity group-hover/tab:opacity-100 hover:bg-white/10'
                                        onClick={(e) => {
                                            e.stopPropagation();
                                            closeSession(session.id);
                                        }}
                                        onKeyDown={(e) => {
                                            if (e.key === "Enter" || e.key === " ") {
                                                e.stopPropagation();
                                                closeSession(session.id);
                                            }
                                        }}
                                        role='button'
                                        tabIndex={0}
                                    >
                                        <X className='size-3' />
                                    </span>
                                </TabsTrigger>
                            );
                        })}
                    </TabsList>

                    {/* Panel-level actions */}
                    <div className='ml-auto flex items-center gap-1 pr-2'>
                        {sessions.length > 1 && (
                            <Button
                                variant='ghost'
                                size='sm'
                                className='size-7 p-0 text-white/60 hover:bg-white/10 hover:text-white'
                                onClick={closeAllSessions}
                                title='Close all sessions'
                            >
                                <XCircle className='size-3.5' />
                            </Button>
                        )}
                        <Button
                            variant='ghost'
                            size='sm'
                            className='size-7 p-0 text-white/60 hover:bg-white/10 hover:text-white'
                            onClick={() => setIsMinimized(!isMinimized)}
                            title={isMinimized ? "Expand" : "Minimize"}
                        >
                            {isMinimized ? <ChevronUp className='size-3.5' /> : <ChevronDown className='size-3.5' />}
                        </Button>
                        {!isMinimized && (
                            <Button
                                variant='ghost'
                                size='sm'
                                className='size-7 p-0 text-white/60 hover:bg-white/10 hover:text-white'
                                onClick={() => setIsFullscreen(!isFullscreen)}
                            >
                                {isFullscreen ? <Minimize2 className='size-3.5' /> : <Maximize2 className='size-3.5' />}
                            </Button>
                        )}
                    </div>
                </div>

                {/* Tab content panels — hidden when minimized but kept mounted to preserve WebSocket connections */}
                {sessions.map((session) => (
                    <TabsContent key={session.id} value={session.id} keepMounted className={isMinimized ? "hidden" : 'min-h-0 flex-1 overflow-hidden'}>
                        <TerminalSessionView target={session.target} onStatusChange={(s) => handleStatusChange(session.id, s)} />
                    </TabsContent>
                ))}
            </Tabs>
        </div>
    );
}
