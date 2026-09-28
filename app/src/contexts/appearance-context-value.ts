import { createContext } from "react";
import type { ColorScheme } from "@/types/settings";

export interface AppearanceContextValue {
    scheme: ColorScheme;
    setScheme: (scheme: ColorScheme) => void;
    resolvedScheme: "light" | "dark";
}

export const AppearanceContext = createContext<AppearanceContextValue | null>(null);
