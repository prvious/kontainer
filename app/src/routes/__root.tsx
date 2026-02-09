import { Outlet, createRootRouteWithContext } from "@tanstack/react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
// import { TanStackDevtools } from "@tanstack/react-devtools";
// import { ReactQueryDevtoolsPanel } from "@tanstack/react-query-devtools";
// import { TanStackRouterDevtoolsPanel } from "@tanstack/react-router-devtools";
import { AppearanceProvider } from "@/contexts/appearance-context";
import { SettingsProvider } from "@/contexts/settings-context";
import type { AuthContextValue } from "@/contexts/auth-context-value";

export interface RouterContext {
    auth: AuthContextValue;
}

const queryClient = new QueryClient();

export const Route = createRootRouteWithContext<RouterContext>()({
    component: () => (
        <AppearanceProvider>
            <SettingsProvider>
                <QueryClientProvider client={queryClient}>
                    <Outlet />
                    {/* <TanStackDevtools
                        config={{
                            position: "top-left",
                        }}
                        plugins={[
                            { name: "TanStack Query", render: <ReactQueryDevtoolsPanel />, defaultOpen: true },
                            { name: "TanStack Router", render: <TanStackRouterDevtoolsPanel /> },
                        ]}
                    /> */}
                </QueryClientProvider>
            </SettingsProvider>
        </AppearanceProvider>
    ),
});
