import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppearanceContext } from "@/contexts/appearance-context-value";
import { useAppSettings } from "@/hooks/use-app-settings";
import { colorSchemeFrom } from "@/types/settings";

/** The OS appearance as the native window reports it; dark until it answers (the design's default mode). */
function useOsColorScheme(): "light" | "dark" {
    const [osScheme, setOsScheme] = useState<"light" | "dark">("dark");
    useEffect(() => {
        const window = getCurrentWindow();
        let active = true;
        window
            .theme()
            .then((theme) => {
                if (active) setOsScheme(theme === "light" ? "light" : "dark");
            })
            .catch(() => {});
        const unlisten = window.onThemeChanged(({ payload }) => setOsScheme(payload === "light" ? "light" : "dark"));
        return () => {
            active = false;
            unlisten.then((stop) => stop()).catch(() => {});
        };
    }, []);
    return osScheme;
}

export function AppearanceProvider({ children }: { children: React.ReactNode }) {
    const { data: settings } = useAppSettings();
    const scheme = colorSchemeFrom(settings?.scheme);
    const osScheme = useOsColorScheme();
    const resolvedScheme = scheme === "system" ? osScheme : scheme;

    // The palettes in app.css switch on the `.dark` class of <html>.
    useEffect(() => {
        document.documentElement.classList.toggle("dark", resolvedScheme === "dark");
    }, [resolvedScheme]);

    // Keep the native title bar and traffic lights in the same appearance.
    useEffect(() => {
        if (!settings) return;
        getCurrentWindow()
            .setTheme(scheme === "system" ? null : scheme)
            .catch(() => {});
    }, [settings, scheme]);

    return <AppearanceContext value={{ scheme, resolvedScheme }}>{children}</AppearanceContext>;
}
