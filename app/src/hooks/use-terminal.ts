import { use } from "react";
import { TerminalContext } from "@/contexts/terminal-context-value";
import type { TerminalContextValue } from "@/contexts/terminal-context-value";

export function useTerminal(): TerminalContextValue {
    const context = use(TerminalContext);
    if (!context) {
        throw new Error("useTerminal must be used within a TerminalProvider");
    }
    return context;
}
