import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Theme } from "@tauri-apps/api/window";
import { themes as availableThemes } from "@/lib/themes";
import type { Scheme } from "@/lib/themes";
import { fonts as availableFonts, DEFAULT_FONT_ID, getFontById } from "@/lib/fonts";
import type { AppSettings } from "@/types/settings";
import { AppearanceContext } from "@/contexts/appearance-context-value";

function syncDOM(themeId: string, fontCssFamily: string) {
    const cl = document.documentElement.classList;

    cl.add("dark");

    // Remove any existing theme classes, then add the new one (skip "default")
    for (const c of Array.from(cl)) {
        if (c.startsWith("theme-")) cl.remove(c);
    }
    if (themeId !== "default") {
        cl.add(`theme-${themeId}`);
    }

    document.documentElement.style.setProperty("--font-sans", fontCssFamily);
}

export function AppearanceProvider({ children }: { children: React.ReactNode }) {
    const [scheme, setSchemeState] = useState<Scheme>("system");
    const [osTheme, setOsTheme] = useState<"light" | "dark">("light");
    const [theme, setThemeState] = useState("default");
    const [font, setFontState] = useState(DEFAULT_FONT_ID);
    const loadedFonts = useRef(new Set([DEFAULT_FONT_ID]));
    const [ready, setReady] = useState(false);

    const resolved: "light" | "dark" = scheme === "system" ? osTheme : scheme;

    // Load saved settings on mount and detect OS theme
    useEffect(() => {
        (async () => {
            let savedScheme: Scheme | null = null;
            let savedFont: string | null = null;

            try {
                const settings = await invoke<AppSettings>("load_app_settings");
                savedScheme = (settings.scheme as Scheme) ?? null;
                savedFont = settings.font ?? null;
            } catch {
                // ignore — use defaults
            }

            try {
                const currentOsTheme = await getCurrentWindow().theme();
                setOsTheme(currentOsTheme === "dark" ? "dark" : "light");
            } catch {
                // ignore — keep "light" default
            }

            if (savedScheme) setSchemeState(savedScheme);
            if (savedFont) setFontState(savedFont);
            setReady(true);
        })();
    }, []);

    // Listen to OS theme changes via Tauri's native API
    useEffect(() => {
        const promise = getCurrentWindow().onThemeChanged(({ payload }: { payload: Theme }) => {
            setOsTheme(payload === "dark" ? "dark" : "light");
        });

        return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
        };
    }, []);

    // Sync native window title bar theme
    useEffect(() => {
        if (!ready) return;
        const nativeTheme = scheme === "system" ? null : scheme;
        getCurrentWindow()
            .setTheme(nativeTheme)
            .catch(() => {});
    }, [ready, scheme]);

    const setScheme = useCallback((next: Scheme) => {
        setSchemeState(next);
        // Persist to app settings
        invoke<AppSettings>("load_app_settings")
            .then((current) => invoke("save_app_settings", { settings: { ...current, scheme: next } }))
            .catch(() => {});
    }, []);

    const setTheme = useCallback((next: string) => {
        setThemeState(next);
    }, []);

    const setFont = useCallback((nextFontId: string) => {
        setFontState(nextFontId);
        // Persist to app settings
        invoke<AppSettings>("load_app_settings")
            .then((current) => invoke("save_app_settings", { settings: { ...current, font: nextFontId } }))
            .catch(() => {});
    }, []);

    // Load font CSS and sync DOM whenever font, resolved scheme, or theme changes
    useEffect(() => {
        if (!ready) return;

        const def = getFontById(font);

        if (loadedFonts.current.has(font)) {
            syncDOM(theme, def.cssFamily);
            return;
        }

        let cancelled = false;
        def.load()
            .then(() => {
                if (cancelled) return;
                loadedFonts.current.add(font);
                syncDOM(theme, def.cssFamily);
            })
            .catch(() => {
                if (cancelled) return;
                syncDOM(theme, def.cssFamily);
            });

        return () => {
            cancelled = true;
        };
    }, [font, resolved, theme, ready]);

    const value = useMemo(
        () => ({
            scheme,
            setScheme,
            resolvedScheme: resolved,
            theme,
            setTheme,
            themes: availableThemes,
            font,
            setFont,
            fonts: availableFonts,
            isFontLoading: false,
        }),
        [scheme, setScheme, resolved, theme, setTheme, font, setFont],
    );

    return <AppearanceContext value={value}>{children}</AppearanceContext>;
}
