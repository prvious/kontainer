import { use } from "react";
import { AppearanceContext } from "@/contexts/appearance-context-value";
import type { AppearanceContextValue } from "@/contexts/appearance-context-value";

export function useAppearance(): AppearanceContextValue {
    const context = use(AppearanceContext);
    if (!context) {
        throw new Error("useAppearance must be used within an AppearanceProvider");
    }
    return context;
}
