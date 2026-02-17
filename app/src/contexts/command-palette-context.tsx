import { useState } from "react";
import { CommandPaletteContext } from "@/contexts/command-palette-context-value";

export function CommandPaletteProvider({ children }: { children: React.ReactNode }) {
    const [open, setOpen] = useState(false);
    const [initialSearch, setInitialSearch] = useState("");

    return <CommandPaletteContext value={{ open, setOpen, initialSearch, setInitialSearch }}>{children}</CommandPaletteContext>;
}
