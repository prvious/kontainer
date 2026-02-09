import { createContext } from "react";

export interface TerminalDefaults {
    shell: string;
    fontSize: number;
    fontFamily: string;
    scrollback: number;
}

export interface SettingsContextValue {
    refreshInterval: number | false;
    setRefreshInterval: (value: number | false) => void;
    terminalDefaults: TerminalDefaults;
    setTerminalDefaults: (value: Partial<TerminalDefaults>) => void;
    isLoading: boolean;
}

export const SettingsContext = createContext<SettingsContextValue | null>(null);
