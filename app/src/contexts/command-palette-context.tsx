import { useState } from "react";
import { CommandPaletteContext } from "@/contexts/command-palette-context-value";

export function CommandPaletteProvider({ children }: { children: React.ReactNode }) {
    const [open, setOpen] = useState(false);

    return <CommandPaletteContext value={{ open, setOpen }}>{children}</CommandPaletteContext>;
}
