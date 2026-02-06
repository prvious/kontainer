import { createContext } from "react";
import type { Scheme, ThemeDefinition } from "@/lib/themes";

export interface AppearanceContextValue {
    scheme: Scheme;
    setScheme: (scheme: Scheme) => void;
    resolvedScheme: "light" | "dark";
    theme: string;
    setTheme: (theme: string) => void;
    themes: ThemeDefinition[];
}

export const AppearanceContext = createContext<AppearanceContextValue | null>(null);
