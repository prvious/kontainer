export type Scheme = "light" | "dark" | "system";

export interface ThemeDefinition {
    id: string;
    label: string;
}

export const themes: ThemeDefinition[] = [{ id: "default", label: "Default" }];
