import { useNavigate } from "@tanstack/react-router";
import { notify } from "@/components/kit/notify";
import { useAuth } from "@/hooks/use-auth";

/**
 * Sign out, then open Connect with another agent's origin filled in. Signing out ends every
 * shell and log stream in Rust. Inside the signed-in shell, its signed-out redirect carries the
 * origin; the navigation here covers a switcher rendered outside it. Resolves false, after
 * saying so, when signing out failed.
 */
export function useSwitchAgent() {
    const { logout } = useAuth();
    const navigate = useNavigate();
    return async (origin: string): Promise<boolean> => {
        try {
            await logout(undefined, origin);
        } catch (code) {
            notify.error("Couldn't sign out", { code: String(code), message: "You're still signed in to the current agent." });
            return false;
        }
        void navigate({ to: "/login", search: { redirect: undefined, origin } });
        return true;
    };
}
