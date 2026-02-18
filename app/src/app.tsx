import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RouterProvider } from "@tanstack/react-router";
import { AuthProvider } from "@/contexts/auth-context";
import { useAuth } from "@/hooks/use-auth";
import { useNativeBehavior } from "@/hooks/use-native-behavior";
import { setAuthErrorHandler } from "@/lib/query-client";
import { router } from "@/router";
import { createRoot } from "react-dom/client";
import "./app.css";

function InnerApp() {
    const auth = useAuth();
    useNativeBehavior();

    useEffect(() => {
        if (!auth.isLoading) {
            invoke("close_splashscreen");
        }
    }, [auth.isLoading]);

    useEffect(() => {
        setAuthErrorHandler(() => {
            auth.logout().then(() => router.navigate({ to: "/login", search: { redirect: undefined } }));
        });
        return () => setAuthErrorHandler(null);
    }, [auth.logout]);

    if (auth.isLoading) {
        return null;
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
