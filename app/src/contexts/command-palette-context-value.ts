import { createContext } from "react";

export interface CommandPaletteContextValue {
    open: boolean;
    setOpen: (open: boolean) => void;
    initialSearch: string;
    setInitialSearch: (search: string) => void;
}

export const CommandPaletteContext = createContext<CommandPaletteContextValue | null>(null);
