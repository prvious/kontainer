import { Navigate, createFileRoute, redirect } from "@tanstack/react-router";
import { AccentLine } from "@/components/accent-line";
import { ConnectCard } from "@/components/connect/connect-card";
import { Titlebar } from "@/components/shell/titlebar";
import { useAuth } from "@/hooks/use-auth";

interface LoginSearch {
    redirect?: string;
    /** Prefill for the agent URL (cluster switcher, settings); "" means an empty field. */
    origin?: string;
}

export const Route = createFileRoute("/login")({
    validateSearch: (search: Record<string, unknown>): LoginSearch => ({
        redirect: typeof search.redirect === "string" && search.redirect ? search.redirect : undefined,
        origin: typeof search.origin === "string" ? search.origin : undefined,
    }),
    beforeLoad: ({ context, search }) => {
        if (context.auth.session) {
            throw redirect({ to: search.redirect || "/" });
        }
    },
    component: LoginLayout,
});

function LoginLayout() {
    const { redirect: redirectTo, origin } = Route.useSearch();
    const { session } = useAuth();
    // Rendering the redirect (rather than navigating from the submit handler) lets the router
    // see the new session in its context before the authenticated guard runs.
    if (session) return <Navigate to={redirectTo || "/"} />;
    return (
        <div className='flex h-screen flex-col bg-background'>
            <AccentLine />
            <Titlebar />
            <ConnectCard
                // A new prefill (switching agents) starts a fresh card.
                key={origin ?? ""}
                initialOrigin={origin}
            />
        </div>
    );
}
