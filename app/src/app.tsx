import { RouterProvider } from "@tanstack/react-router";
import { AuthProvider } from "@/contexts/auth-context";
import { useAuth } from "@/hooks/use-auth";
import { router } from "@/router";

function InnerApp() {
    const auth = useAuth();
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
