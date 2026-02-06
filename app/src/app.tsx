import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RouterProvider } from "@tanstack/react-router";
import { AuthProvider } from "@/contexts/auth-context";
import { useAuth } from "@/hooks/use-auth";
import { router } from "@/router";

function InnerApp() {
    const auth = useAuth();

    useEffect(() => {
        if (!auth.isLoading) {
            invoke("close_splashscreen");
        }
    }, [auth.isLoading]);

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

export default App;
