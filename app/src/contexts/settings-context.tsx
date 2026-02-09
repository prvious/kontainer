import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AppSettings } from "@/types/settings";
import { SettingsContext } from "@/contexts/settings-context-value";

export function SettingsProvider({ children }: { children: React.ReactNode }) {
    const [refreshInterval, setRefreshIntervalState] = useState<number | false>(5000);
    const [isLoading, setIsLoading] = useState(true);

    useEffect(() => {
        let cancelled = false;

        async function load() {
            try {
                const settings = await invoke<AppSettings>("load_app_settings");
                if (cancelled) return;
                setRefreshIntervalState(settings.refreshInterval === null ? false : settings.refreshInterval);
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

    const setRefreshInterval = useCallback((value: number | false) => {
        setRefreshIntervalState(value);
        invoke("save_app_settings", {
            settings: { refreshInterval: value === false ? null : value },
        }).catch((err) => console.error("Failed to save app settings:", err));
    }, []);

    const value = useMemo(() => ({ refreshInterval, setRefreshInterval, isLoading }), [refreshInterval, setRefreshInterval, isLoading]);

    return <SettingsContext value={value}>{children}</SettingsContext>;
}
