import { useCallback, useRef, useState } from "react";
import { TerminalSessionView } from "@/components/terminal-session-view";
import { X, Minus, Maximize2 } from "lucide-react";
import { useTerminal } from "@/hooks/use-terminal";

const MIN_HEIGHT = 150;
const DEFAULT_HEIGHT = 300;
const MAX_HEIGHT_VH = 0.8;

export function TerminalPanel() {
    const { sessions, activeSessionId, isMinimized, closeAllSessions, setMinimized } = useTerminal();
    const [panelHeight, setPanelHeight] = useState(DEFAULT_HEIGHT);
    const [isFullscreen, setIsFullscreen] = useState(false);
    const isDraggingRef = useRef(false);

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
        ? "fixed bottom-9 left-2 right-2 top-(--titlebar-inset) z-50 flex flex-col rounded-xl border border-border bg-background p-2 shadow-[0_-4px_20px_rgba(0,0,0,0.3)]"
        : "fixed bottom-9 left-2 right-2 z-40 flex flex-col rounded-xl border border-border bg-background p-2 shadow-[0_-4px_20px_rgba(0,0,0,0.3)]";

    const panelStyle = isFullscreen ? undefined : { height: panelHeight };

    return (
        <div className={isMinimized ? 'invisible fixed -bottom-full' : panelClasses} style={isMinimized ? undefined : panelStyle}>
            {/* Resize handle */}
            {!isFullscreen && <div className='h-1 cursor-row-resize transition-colors hover:bg-white/10' onMouseDown={handleMouseDown} />}

            {/* Terminal content — show active session only, keep others mounted */}
            <div className='relative min-h-0 flex-1 overflow-hidden'>
                {sessions.map((session) => (
                    <div key={session.id} className={session.id === activeSessionId ? "size-full" : "hidden"}>
                        <TerminalSessionView target={session.target} onStatusChange={() => {}} />
                    </div>
                ))}

                {/* Traffic light controls */}
                <div className='group/lights absolute top-2 right-3 z-10 flex items-center gap-1.5'>
                    <button
                        className='flex size-3 items-center justify-center rounded-full bg-[#28c840] hover:brightness-110 active:brightness-90'
                        onClick={() => setIsFullscreen(!isFullscreen)}
                        title={isFullscreen ? "Exit fullscreen" : "Fullscreen"}
                    >
                        <Maximize2 className='size-1.5 text-black/0 group-hover/lights:text-black/80' strokeWidth={3} />
                    </button>
                    <button
                        className='flex size-3 items-center justify-center rounded-full bg-[#febc2e] hover:brightness-110 active:brightness-90'
                        onClick={() => setMinimized(true)}
                        title='Minimize'
                    >
                        <Minus className='size-2 text-black/0 group-hover/lights:text-black/80' strokeWidth={3} />
                    </button>
                    <button
                        className='flex size-3 items-center justify-center rounded-full bg-[#ff5f57] hover:brightness-110 active:brightness-90'
                        onClick={closeAllSessions}
                        title='Close all sessions'
                    >
                        <X className='size-2 text-black/0 group-hover/lights:text-black/80' strokeWidth={3} />
                    </button>
                </div>
            </div>
        </div>
    );
}
