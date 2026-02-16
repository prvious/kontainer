import { useCallback, useEffect, useRef } from "react";
import { Terminal, FitAddon, init as initGhostty } from "ghostty-web";
import { useExecStream } from "@/hooks/use-exec-stream";
import { useSettings } from "@/hooks/use-settings";
import { Button } from "@/components/ui/button";
import { RotateCw } from "lucide-react";
import type { ExecTarget } from "@/contexts/terminal-context-value";
import type { ExecStatus } from "@/hooks/use-exec-stream";

interface TerminalSessionViewProps {
    target: ExecTarget;
    onStatusChange: (status: ExecStatus) => void;
}

export function TerminalSessionView({ target, onStatusChange }: TerminalSessionViewProps) {
    const termContainerRef = useRef<HTMLDivElement>(null);
    const terminalRef = useRef<Terminal | null>(null);
    const fitAddonRef = useRef<FitAddon | null>(null);
    const onStatusChangeRef = useRef(onStatusChange);
    onStatusChangeRef.current = onStatusChange;

    const { terminalDefaults } = useSettings();

    const effectiveShell = target.shell || terminalDefaults.shell || undefined;
    const { status, attachTerminal, sendInput, sendResize, connect, disconnect } = useExecStream(
        target.containerId,
        target.nodeId,
        effectiveShell,
        target.user,
    );

    // Report status changes to parent (ref avoids re-firing on callback identity change)
    useEffect(() => {
        onStatusChangeRef.current(status);
    }, [status]);

    // Initialize terminal
    useEffect(() => {
        const ghosttyReady = initGhostty();

        if (!termContainerRef.current) return;
        const container = termContainerRef.current;
        let cancelled = false;
        let observer: ResizeObserver | null = null;
        let term: Terminal | null = null;

        ghosttyReady.then(() => {
            if (cancelled) return;

            container.replaceChildren();

            term = new Terminal({
                cursorBlink: true,
                fontSize: terminalDefaults.fontSize,
                fontFamily: terminalDefaults.fontFamily,
                scrollback: terminalDefaults.scrollback,
                theme: {
                    background: "#08090b",
                    foreground: "#e0e0e0",
                    cursor: "#e0e0e0",
                    selectionBackground: "#3a3b46",
                    black: "#18191f",
                    red: "#e5706b",
                    green: "#8fbf6a",
                    yellow: "#d4a855",
                    blue: "#6b9ef7",
                    magenta: "#b08def",
                    cyan: "#6bc4d6",
                    white: "#c8c8c8",
                    brightBlack: "#4a4b56",
                    brightRed: "#f08a85",
                    brightGreen: "#a3d47e",
                    brightYellow: "#e0be6f",
                    brightBlue: "#85b4f7",
                    brightMagenta: "#c4a7f5",
                    brightCyan: "#85d8ea",
                    brightWhite: "#eeeeee",
                },
            });

            const fitAddon = new FitAddon();
            term.loadAddon(fitAddon);
            term.open(container);

            requestAnimationFrame(() => {
                fitAddon.fit();
            });

            term.onData((data) => sendInput(data));

            terminalRef.current = term;
            fitAddonRef.current = fitAddon;
            attachTerminal(term);

            const cols = term.cols;
            const rows = term.rows;
            connect(cols, rows);

            // ResizeObserver — also fires when panel transitions from hidden to visible
            observer = new ResizeObserver(() => {
                requestAnimationFrame(() => {
                    fitAddon.fit();
                    sendResize(term!.cols, term!.rows);
                });
            });
            observer.observe(container);
        });

        return () => {
            cancelled = true;
            observer?.disconnect();
            disconnect();
            term?.dispose();
            terminalRef.current = null;
            fitAddonRef.current = null;
        };
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [target.containerId, target.nodeId, terminalDefaults]);

    const handleReconnect = useCallback(() => {
        terminalRef.current?.clear();
        const cols = terminalRef.current?.cols ?? 80;
        const rows = terminalRef.current?.rows ?? 24;
        connect(cols, rows);
    }, [connect]);

    const showReconnect = status === "disconnected" || status === "error" || status === "exited";

    return (
        <div className='relative size-full'>
            <div ref={termContainerRef} className='size-full p-1' />
            {showReconnect && (
                <div className='absolute inset-0 flex items-center justify-center bg-[#08090b]/80'>
                    <Button
                        variant='outline'
                        size='sm'
                        className='gap-2 border-white/20 text-white/80 hover:bg-white/10 hover:text-white'
                        onClick={handleReconnect}
                    >
                        <RotateCw className='size-3.5' />
                        Reconnect
                    </Button>
                </div>
            )}
        </div>
    );
}
