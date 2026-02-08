import { useCallback, useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { useExecStream } from "@/hooks/use-exec-stream";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Maximize2, Minimize2, RotateCw, X } from "lucide-react";
import type { ExecTarget } from "@/contexts/terminal-context-value";
import "@xterm/xterm/css/xterm.css";

const MIN_HEIGHT = 150;
const DEFAULT_HEIGHT = 300;
const MAX_HEIGHT_VH = 0.8;

const statusLabels: Record<string, string> = {
    disconnected: "Disconnected",
    connecting: "Connecting...",
    connected: "Connected",
    error: "Error",
    exited: "Exited",
};

const statusVariants: Record<string, "default" | "secondary" | "destructive"> = {
    disconnected: "secondary",
    connecting: "secondary",
    connected: "default",
    error: "destructive",
    exited: "secondary",
};

export function TerminalPanel({ target, onClose }: { target: ExecTarget; onClose: () => void }) {
    const termContainerRef = useRef<HTMLDivElement>(null);
    const terminalRef = useRef<Terminal | null>(null);
    const fitAddonRef = useRef<FitAddon | null>(null);
    const [panelHeight, setPanelHeight] = useState(DEFAULT_HEIGHT);
    const [isFullscreen, setIsFullscreen] = useState(false);
    const isDraggingRef = useRef(false);

    const { status, attachTerminal, sendInput, sendResize, connect, disconnect } = useExecStream(target.containerId, target.nodeId, target.shell);

    // Initialize terminal
    useEffect(() => {
        if (!termContainerRef.current) return;

        const term = new Terminal({
            cursorBlink: true,
            fontSize: 13,
            fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', Menlo, Monaco, 'Courier New', monospace",
            theme: {
                background: "#1a1b26",
                foreground: "#c0caf5",
                cursor: "#c0caf5",
                selectionBackground: "#33467c",
                black: "#15161e",
                red: "#f7768e",
                green: "#9ece6a",
                yellow: "#e0af68",
                blue: "#7aa2f7",
                magenta: "#bb9af7",
                cyan: "#7dcfff",
                white: "#a9b1d6",
                brightBlack: "#414868",
                brightRed: "#f7768e",
                brightGreen: "#9ece6a",
                brightYellow: "#e0af68",
                brightBlue: "#7aa2f7",
                brightMagenta: "#bb9af7",
                brightCyan: "#7dcfff",
                brightWhite: "#c0caf5",
            },
        });

        const fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        term.open(termContainerRef.current);

        // Small delay so DOM has time to settle dimensions
        requestAnimationFrame(() => {
            fitAddon.fit();
        });

        term.onData((data) => sendInput(data));

        terminalRef.current = term;
        fitAddonRef.current = fitAddon;
        attachTerminal(term);

        // Connect after terminal is ready
        const cols = term.cols;
        const rows = term.rows;
        connect(cols, rows);

        // ResizeObserver
        const observer = new ResizeObserver(() => {
            requestAnimationFrame(() => {
                fitAddon.fit();
                sendResize(term.cols, term.rows);
            });
        });
        observer.observe(termContainerRef.current);

        return () => {
            observer.disconnect();
            disconnect();
            term.dispose();
            terminalRef.current = null;
            fitAddonRef.current = null;
        };
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [target.containerId, target.nodeId]);

    // Re-fit on height/fullscreen changes
    useEffect(() => {
        requestAnimationFrame(() => {
            fitAddonRef.current?.fit();
            if (terminalRef.current) {
                sendResize(terminalRef.current.cols, terminalRef.current.rows);
            }
        });
    }, [panelHeight, isFullscreen, sendResize]);

    const handleReconnect = useCallback(() => {
        terminalRef.current?.clear();
        const cols = terminalRef.current?.cols ?? 80;
        const rows = terminalRef.current?.rows ?? 24;
        connect(cols, rows);
    }, [connect]);

    // Drag resize
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

    const containerName = target.containerName || target.containerId.slice(0, 12);

    const panelClasses = isFullscreen
        ? "fixed inset-0 z-50 flex flex-col bg-[#1a1b26]"
        : "fixed bottom-0 left-0 right-0 z-40 flex flex-col border-t border-border bg-[#1a1b26] shadow-[0_-4px_20px_rgba(0,0,0,0.3)]";

    return (
        <div className={panelClasses} style={isFullscreen ? undefined : { height: panelHeight }}>
            {/* Resize handle */}
            {!isFullscreen && <div className='h-1 cursor-row-resize bg-border transition-colors hover:bg-primary/50' onMouseDown={handleMouseDown} />}

            {/* Toolbar */}
            <div className='flex items-center justify-between border-b border-white/10 px-3 py-1.5'>
                <div className='flex items-center gap-2'>
                    <span className='text-xs font-medium text-white/80'>{containerName}</span>
                    <Badge variant={statusVariants[status]}>{statusLabels[status]}</Badge>
                </div>
                <div className='flex items-center gap-1'>
                    {(status === "disconnected" || status === "error" || status === "exited") && (
                        <Button variant='ghost' size='sm' className='size-7 p-0 text-white/60 hover:bg-white/10 hover:text-white' onClick={handleReconnect}>
                            <RotateCw className='size-3.5' />
                        </Button>
                    )}
                    <Button
                        variant='ghost'
                        size='sm'
                        className='size-7 p-0 text-white/60 hover:bg-white/10 hover:text-white'
                        onClick={() => setIsFullscreen(!isFullscreen)}
                    >
                        {isFullscreen ? <Minimize2 className='size-3.5' /> : <Maximize2 className='size-3.5' />}
                    </Button>
                    <Button variant='ghost' size='sm' className='size-7 p-0 text-white/60 hover:bg-white/10 hover:text-white' onClick={onClose}>
                        <X className='size-3.5' />
                    </Button>
                </div>
            </div>

            {/* Terminal */}
            <div ref={termContainerRef} className='flex-1 overflow-hidden p-1' />
        </div>
    );
}
