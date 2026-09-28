import { useAppSettings, useSaveAppSettings } from "@/hooks/use-app-settings";
import { DEFAULT_APP_SETTINGS } from "@/types/settings";

export interface TerminalDefaults {
    shell: string;
    fontSize: number;
    fontFamily: string;
    scrollback: number;
}

const DEFAULT_TERMINAL: TerminalDefaults = {
    shell: "",
    fontSize: 13,
    fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', Menlo, Monaco, 'Courier New', monospace",
    scrollback: 1000,
};

/** Polling interval and terminal defaults derived from the stored app settings. */
export function useSettings() {
    const { data, isPending } = useAppSettings();
    const save = useSaveAppSettings();
    const settings = data ?? DEFAULT_APP_SETTINGS;
    const terminalDefaults: TerminalDefaults = {
        shell: settings.terminalShell ?? DEFAULT_TERMINAL.shell,
        fontSize: settings.terminalFontSize ?? DEFAULT_TERMINAL.fontSize,
        fontFamily: settings.terminalFontFamily ?? DEFAULT_TERMINAL.fontFamily,
        scrollback: settings.terminalScrollback ?? DEFAULT_TERMINAL.scrollback,
    };

    return {
        refreshInterval: settings.refreshInterval === null ? (false as const) : settings.refreshInterval,
        setRefreshInterval: (value: number | false) => save.mutate({ ...settings, refreshInterval: value === false ? null : value }),
        terminalDefaults,
        setTerminalDefaults: (value: Partial<TerminalDefaults>) => {
            const next = { ...terminalDefaults, ...value };
            save.mutate({
                ...settings,
                terminalShell: next.shell || null,
                terminalFontSize: next.fontSize === DEFAULT_TERMINAL.fontSize ? null : next.fontSize,
                terminalFontFamily: next.fontFamily === DEFAULT_TERMINAL.fontFamily ? null : next.fontFamily,
                terminalScrollback: next.scrollback === DEFAULT_TERMINAL.scrollback ? null : next.scrollback,
            });
        },
        isLoading: isPending,
    };
}
