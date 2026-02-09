import { use } from "react";
import { SettingsContext } from "@/contexts/settings-context-value";
import type { SettingsContextValue } from "@/contexts/settings-context-value";

export function useSettings(): SettingsContextValue {
    const context = use(SettingsContext);
    if (!context) {
        throw new Error("useSettings must be used within a SettingsProvider");
    }
    return context;
}
