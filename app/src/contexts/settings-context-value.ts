import { createContext } from "react";

export interface SettingsContextValue {
    refreshInterval: number | false;
    setRefreshInterval: (value: number | false) => void;
    isLoading: boolean;
}

export const SettingsContext = createContext<SettingsContextValue | null>(null);
