/**
 * Local desktop preferences stored by Tauri in `settings.json`.
 * Mirrors `AppSettings` in `app/src-tauri/src/actions/app/load_app_settings.rs`.
 */
export interface AppSettings {
    refreshInterval: number | null;
    terminalShell: string | null;
    terminalFontSize: number | null;
    terminalFontFamily: string | null;
    terminalScrollback: number | null;
    scheme: string | null;
}

export type ColorScheme = "system" | "light" | "dark";

export const DEFAULT_APP_SETTINGS: AppSettings = {
    refreshInterval: 5000,
    terminalShell: null,
    terminalFontSize: null,
    terminalFontFamily: null,
    terminalScrollback: null,
    scheme: null,
};

export function colorSchemeFrom(value: string | null | undefined): ColorScheme {
    return value === "light" || value === "dark" ? value : "system";
}

export interface TerminalDefaults {
    shell: string;
    fontSize: number;
    fontFamily: string;
    scrollback: number;
}

/** What a terminal uses for a setting left unset. */
export const DEFAULT_TERMINAL: TerminalDefaults = {
    shell: "",
    fontSize: 13,
    fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', Menlo, Monaco, 'Courier New', monospace",
    scrollback: 1000,
};
