import { createContext } from "react";
import type { Scheme, ThemeDefinition } from "@/lib/themes";
import type { FontDefinition } from "@/lib/fonts";

export interface AppearanceContextValue {
    scheme: Scheme;
    setScheme: (scheme: Scheme) => void;
    resolvedScheme: "light" | "dark";
    theme: string;
    setTheme: (theme: string) => void;
    themes: ThemeDefinition[];
    font: string;
    setFont: (fontId: string) => void;
    fonts: FontDefinition[];
    isFontLoading: boolean;
}

export const AppearanceContext = createContext<AppearanceContextValue | null>(null);
