import { useAppSettings } from "@/hooks/use-app-settings";
import { DEFAULT_APP_SETTINGS, DEFAULT_TERMINAL, type TerminalDefaults } from "@/types/settings";

/** Terminal defaults derived from the stored app settings. */
export function useSettings() {
    const { data } = useAppSettings();
    const settings = data ?? DEFAULT_APP_SETTINGS;
    const terminalDefaults: TerminalDefaults = {
        shell: settings.terminalShell ?? DEFAULT_TERMINAL.shell,
        fontSize: settings.terminalFontSize ?? DEFAULT_TERMINAL.fontSize,
        fontFamily: settings.terminalFontFamily ?? DEFAULT_TERMINAL.fontFamily,
        scrollback: settings.terminalScrollback ?? DEFAULT_TERMINAL.scrollback,
    };

    return { terminalDefaults };
}
