import { useEffect, useEffectEvent, useRef } from "react";
import { FitAddon, Terminal, init as initGhostty } from "ghostty-web";
import type { ExecTab } from "@/contexts/exec-sessions-context-value";
import { useSettings } from "@/hooks/use-settings";
import { cn } from "@/lib/utils";

/** Hide the cursor: an ended session shows no prompt to type at. */
const HIDE_CURSOR = "\u001b[?25l";

const THEME = {
    background: "#08090b",
    foreground: "#e0e0e0",
    cursor: "#c6f432",
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
};

/**
 * The terminal for one Exec tab (ghostty-web). It replays what the session wrote before it
 * mounted, writes new output as it arrives, sends keystrokes and its size, and freezes once the
 * session ended: input blocked, cursor hidden, output dimmed (08.S1). The terminal is an external
 * system, so its lifetime is an effect.
 */
export function TerminalView({
    tab,
    ended,
    onData,
    onResize,
}: {
    tab: ExecTab;
    ended: boolean;
    onData: (data: string) => void;
    onResize: (cols: number, rows: number) => void;
}) {
    const container = useRef<HTMLDivElement>(null);
    const terminal = useRef<Terminal | null>(null);
    const { terminalDefaults } = useSettings();
    const { fontSize, fontFamily, scrollback } = terminalDefaults;
    const sendData = useEffectEvent((data: string) => {
        if (!ended) onData(data);
    });
    const sendSize = useEffectEvent((cols: number, rows: number) => onResize(cols, rows));
    const freeze = useEffectEvent((term: Terminal) => {
        if (!ended) return;
        term.options.disableStdin = true;
        term.write(HIDE_CURSOR);
    });

    useEffect(() => {
        const element = container.current;
        if (!element) return;
        let disposed = false;
        let term: Terminal | null = null;
        let observer: ResizeObserver | null = null;
        let stopOutput: (() => void) | null = null;
        void initGhostty().then(() => {
            if (disposed) return;
            element.replaceChildren();
            term = new Terminal({ cursorBlink: true, fontSize, fontFamily, scrollback, theme: THEME });
            const fit = new FitAddon();
            term.loadAddon(fit);
            term.open(element);
            for (const chunk of tab.buffer.replay()) term.write(chunk);
            stopOutput = tab.buffer.onOutput((bytes) => term?.write(bytes));
            term.onData((data) => sendData(data));
            const refit = () => {
                if (!term) return;
                fit.fit();
                sendSize(term.cols, term.rows);
            };
            requestAnimationFrame(refit);
            observer = new ResizeObserver(() => requestAnimationFrame(refit));
            observer.observe(element);
            terminal.current = term;
            freeze(term);
        });
        return () => {
            disposed = true;
            observer?.disconnect();
            stopOutput?.();
            term?.dispose();
            terminal.current = null;
        };
    }, [tab.buffer, fontSize, fontFamily, scrollback]);

    useEffect(() => {
        if (terminal.current) freeze(terminal.current);
    }, [ended]);

    return <div ref={container} data-testid='terminal' data-ended={ended || undefined} className={cn("size-full min-h-0 p-2", ended && "opacity-50")} />;
}
