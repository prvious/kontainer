import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { themes as availableThemes } from "@/lib/themes";
import type { Scheme } from "@/lib/themes";
import { fonts as availableFonts, DEFAULT_FONT_ID, getFontById } from "@/lib/fonts";
import type { AppSettings } from "@/types/settings";
import { AppearanceContext } from "@/contexts/appearance-context-value";

const mq = window.matchMedia("(prefers-color-scheme: dark)");

function subscribeToSystemScheme(callback: () => void) {
    mq.addEventListener("change", callback);
    return () => mq.removeEventListener("change", callback);
}

function getSystemSnapshot(): "light" | "dark" {
    return mq.matches ? "dark" : "light";
}

function syncDOM(resolved: "light" | "dark", themeId: string, fontCssFamily: string) {
    const cl = document.documentElement.classList;

    if (resolved === "dark") {
        cl.add("dark");
    } else {
        cl.remove("dark");
    }

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
    const [scheme, setSchemeState] = useState<Scheme>(() => {
        return (localStorage.getItem("scheme") as Scheme) ?? "system";
    });

    const [theme, setThemeState] = useState(() => {
        return localStorage.getItem("theme") ?? "default";
    });

    const [font, setFontState] = useState(DEFAULT_FONT_ID);
    const loadedFonts = useRef(new Set([DEFAULT_FONT_ID]));

    const systemScheme = useSyncExternalStore(subscribeToSystemScheme, getSystemSnapshot);

    const resolved: "light" | "dark" = scheme === "system" ? systemScheme : scheme;

    // Load saved font from app settings on mount
    useEffect(() => {
        invoke<AppSettings>("load_app_settings")
            .then((settings) => {
                if (settings.font) {
                    setFontState(settings.font);
                }
            })
            .catch(() => {});
    }, []);

    const setScheme = useCallback((next: Scheme) => {
        localStorage.setItem("scheme", next);
        setSchemeState(next);
    }, []);

    const setTheme = useCallback((next: string) => {
        localStorage.setItem("theme", next);
        setThemeState(next);
    }, []);

    const setFont = useCallback((nextFontId: string) => {
        setFontState(nextFontId);
        // Persist to app settings
        invoke<AppSettings>("load_app_settings")
            .then((current) => invoke("save_app_settings", { settings: { ...current, font: nextFontId } }))
            .catch(() => {});
    }, []);

    // Load font CSS and sync DOM whenever font, scheme, or theme changes
    useEffect(() => {
        const def = getFontById(font);

        if (loadedFonts.current.has(font)) {
            syncDOM(resolved, theme, def.cssFamily);
            return;
        }

        let cancelled = false;
        def.load()
            .then(() => {
                if (cancelled) return;
                loadedFonts.current.add(font);
                syncDOM(resolved, theme, def.cssFamily);
            })
            .catch(() => {
                if (cancelled) return;
                syncDOM(resolved, theme, def.cssFamily);
            });

        return () => {
            cancelled = true;
        };
    }, [font, resolved, theme]);

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
