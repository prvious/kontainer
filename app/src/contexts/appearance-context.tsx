import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { themes as availableThemes } from "@/lib/themes";
import type { Scheme } from "@/lib/themes";
import { AppearanceContext } from "@/contexts/appearance-context-value";

const mq = window.matchMedia("(prefers-color-scheme: dark)");

function subscribeToSystemScheme(callback: () => void) {
    mq.addEventListener("change", callback);
    return () => mq.removeEventListener("change", callback);
}

function getSystemSnapshot(): "light" | "dark" {
    return mq.matches ? "dark" : "light";
}

function syncDOM(resolved: "light" | "dark", themeId: string) {
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
}

export function AppearanceProvider({ children }: { children: React.ReactNode }) {
    const [scheme, setSchemeState] = useState<Scheme>(() => {
        return (localStorage.getItem("scheme") as Scheme) ?? "system";
    });

    const [theme, setThemeState] = useState(() => {
        return localStorage.getItem("theme") ?? "default";
    });

    const systemScheme = useSyncExternalStore(subscribeToSystemScheme, getSystemSnapshot);

    const resolved: "light" | "dark" = scheme === "system" ? systemScheme : scheme;

    const setScheme = useCallback((next: Scheme) => {
        localStorage.setItem("scheme", next);
        setSchemeState(next);
    }, []);

    const setTheme = useCallback((next: string) => {
        localStorage.setItem("theme", next);
        setThemeState(next);
    }, []);

    // Sync DOM whenever resolved scheme or theme changes
    useEffect(() => {
        syncDOM(resolved, theme);
    }, [resolved, theme]);

    const value = useMemo(
        () => ({
            scheme,
            setScheme,
            resolvedScheme: resolved,
            theme,
            setTheme,
            themes: availableThemes,
        }),
        [scheme, setScheme, resolved, theme, setTheme],
    );

    return <AppearanceContext value={value}>{children}</AppearanceContext>;
}
