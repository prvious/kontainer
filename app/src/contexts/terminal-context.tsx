import { useCallback, useState } from "react";
import { TerminalPanel } from "@/components/terminal-panel";
import { TerminalContext } from "@/contexts/terminal-context-value";
import type { ExecTarget } from "@/contexts/terminal-context-value";

export function TerminalProvider({ children }: { children: React.ReactNode }) {
    const [target, setTarget] = useState<ExecTarget | null>(null);

    const openExec = useCallback((t: ExecTarget) => {
        setTarget(t);
    }, []);

    const closeExec = useCallback(() => {
        setTarget(null);
    }, []);

    return (
        <TerminalContext value={{ target, openExec, closeExec }}>
            {children}
            {target && <TerminalPanel target={target} onClose={closeExec} />}
        </TerminalContext>
    );
}
