import { createContext } from "react";
import type { ColorScheme } from "@/types/settings";

export interface AppearanceContextValue {
    scheme: ColorScheme;
    resolvedScheme: "light" | "dark";
}

export const AppearanceContext = createContext<AppearanceContextValue | null>(null);
