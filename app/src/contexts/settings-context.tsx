import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AppSettings } from "@/types/settings";
import { SettingsContext } from "@/contexts/settings-context-value";
import type { TerminalDefaults } from "@/contexts/settings-context-value";

const DEFAULT_TERMINAL: TerminalDefaults = {
    shell: "",
    fontSize: 13,
    fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', Menlo, Monaco, 'Courier New', monospace",
    scrollback: 1000,
};

export function SettingsProvider({ children }: { children: React.ReactNode }) {
    const [refreshInterval, setRefreshIntervalState] = useState<number | false>(5000);
    const [terminalDefaults, setTerminalDefaultsState] = useState<TerminalDefaults>(DEFAULT_TERMINAL);
    const [isLoading, setIsLoading] = useState(true);

    // Keep a ref so persist helper always sees latest state
    const terminalDefaultsRef = useRef(terminalDefaults);
    terminalDefaultsRef.current = terminalDefaults;
    const refreshIntervalRef = useRef(refreshInterval);
    refreshIntervalRef.current = refreshInterval;

    useEffect(() => {
        let cancelled = false;

        async function load() {
            try {
                const settings = await invoke<AppSettings>("load_app_settings");
                if (cancelled) return;
                setRefreshIntervalState(settings.refreshInterval === null ? false : settings.refreshInterval);
                setTerminalDefaultsState({
                    shell: settings.terminalShell ?? DEFAULT_TERMINAL.shell,
                    fontSize: settings.terminalFontSize ?? DEFAULT_TERMINAL.fontSize,
                    fontFamily: settings.terminalFontFamily ?? DEFAULT_TERMINAL.fontFamily,
                    scrollback: settings.terminalScrollback ?? DEFAULT_TERMINAL.scrollback,
                });
            } catch (err) {
                console.error("Failed to load app settings:", err);
            } finally {
                if (!cancelled) setIsLoading(false);
            }
        }

        load();

        return () => {
            cancelled = true;
        };
    }, []);

    const persist = useCallback((overrides: { refreshInterval?: number | false; terminal?: Partial<TerminalDefaults> }) => {
        const ri = overrides.refreshInterval ?? refreshIntervalRef.current;
        const td = { ...terminalDefaultsRef.current, ...overrides.terminal };
        invoke("save_app_settings", {
            settings: {
                refreshInterval: ri === false ? null : ri,
                terminalShell: td.shell || null,
                terminalFontSize: td.fontSize === DEFAULT_TERMINAL.fontSize ? null : td.fontSize,
                terminalFontFamily: td.fontFamily === DEFAULT_TERMINAL.fontFamily ? null : td.fontFamily,
                terminalScrollback: td.scrollback === DEFAULT_TERMINAL.scrollback ? null : td.scrollback,
                font: null,
                scheme: null,
            } satisfies AppSettings,
        }).catch((err) => console.error("Failed to save app settings:", err));
    }, []);

    const setRefreshInterval = useCallback(
        (value: number | false) => {
            setRefreshIntervalState(value);
            persist({ refreshInterval: value });
        },
        [persist],
    );

    const setTerminalDefaults = useCallback(
        (value: Partial<TerminalDefaults>) => {
            setTerminalDefaultsState((prev) => {
                const next = { ...prev, ...value };
                persist({ terminal: next });
                return next;
            });
        },
        [persist],
    );

    const value = useMemo(
        () => ({ refreshInterval, setRefreshInterval, terminalDefaults, setTerminalDefaults, isLoading }),
        [refreshInterval, setRefreshInterval, terminalDefaults, setTerminalDefaults, isLoading],
    );

    return <SettingsContext value={value}>{children}</SettingsContext>;
}
