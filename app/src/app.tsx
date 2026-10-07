import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RouterProvider } from "@tanstack/react-router";
import { AuthProvider } from "@/contexts/auth-context";
import { AppearanceProvider } from "@/contexts/appearance-context";
import { useAuth } from "@/hooks/use-auth";
import { useNativeBehavior } from "@/hooks/use-native-behavior";
import { router } from "@/router";
import { SplashScreen, STARTUP_FAILURE_CODES, type StartupFailureCode } from "@/components/splash-screen";
import { createRoot } from "react-dom/client";
import "./app.css";

function startupFailure(code: string | null): StartupFailureCode | null {
    return STARTUP_FAILURE_CODES.find((candidate) => candidate === code) ?? null;
}

function InnerApp() {
    const auth = useAuth();
    const [attempt, setAttempt] = useState(0);
    useNativeBehavior();

    // Show the main window as soon as React renders, so the full-size splash (and its failure
    // state) is what the owner sees instead of the small native loading window.
    useEffect(() => {
        void invoke("close_splashscreen").catch(() => {});
    }, []);

    const failure = startupFailure(auth.restoreError);
    if (auth.isLoading || failure) {
        return (
            <AppearanceProvider>
                <SplashScreen
                    key={attempt}
                    failure={failure}
                    onRetry={() => {
                        setAttempt((current) => current + 1);
                        void auth.retryRestore().catch(() => {});
                    }}
                />
            </AppearanceProvider>
        );
    }

    return <RouterProvider router={router} context={{ auth }} />;
}

export function App() {
    return (
        <AuthProvider>
            <InnerApp />
        </AuthProvider>
    );
}

createRoot(document.getElementById("root")!).render(<App />);
