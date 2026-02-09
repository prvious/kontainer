import { useCallback, useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { useExecStream } from "@/hooks/use-exec-stream";
import { useSettings } from "@/hooks/use-settings";
import { Button } from "@/components/ui/button";
import { RotateCw } from "lucide-react";
import type { ExecTarget } from "@/contexts/terminal-context-value";
import type { ExecStatus } from "@/hooks/use-exec-stream";
import "@xterm/xterm/css/xterm.css";

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
        if (!termContainerRef.current) return;

        // Clear leftover DOM nodes — xterm's dispose() does not remove them
        termContainerRef.current.replaceChildren();

        const term = new Terminal({
            cursorBlink: true,
            fontSize: terminalDefaults.fontSize,
            fontFamily: terminalDefaults.fontFamily,
            scrollback: terminalDefaults.scrollback,
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
                <div className='absolute inset-0 flex items-center justify-center bg-[#1a1b26]/80'>
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
