import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RouterProvider } from "@tanstack/react-router";
import { AuthProvider } from "@/contexts/auth-context";
import { useAuth } from "@/hooks/use-auth";
import { useNativeBehavior } from "@/hooks/use-native-behavior";
import { router } from "@/router";
import { SplashScreen } from "@/components/splash-screen";
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

    if (auth.isLoading) {
        return <SplashScreen />;
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
