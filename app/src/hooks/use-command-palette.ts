import { use } from "react";
import { CommandPaletteContext } from "@/contexts/command-palette-context-value";
import type { CommandPaletteContextValue } from "@/contexts/command-palette-context-value";

export function useCommandPalette(): CommandPaletteContextValue {
    const context = use(CommandPaletteContext);
    if (!context) {
        throw new Error("useCommandPalette must be used within a CommandPaletteProvider");
    }
    return context;
}
