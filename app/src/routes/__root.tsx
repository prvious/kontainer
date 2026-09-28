import { Outlet, createRootRouteWithContext } from "@tanstack/react-router";
import { AppearanceProvider } from "@/contexts/appearance-context";
import { AppToaster } from "@/components/kit/app-toaster";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { AuthContextValue } from "@/contexts/auth-context-value";

export interface RouterContext {
    auth: AuthContextValue;
}

export const Route = createRootRouteWithContext<RouterContext>()({
    component: () => (
        <AppearanceProvider>
            {/* Icon-only actions show their label tooltip after 400 ms. */}
            <TooltipProvider delay={400}>
                <Outlet />
                <AppToaster />
            </TooltipProvider>
        </AppearanceProvider>
    ),
});
