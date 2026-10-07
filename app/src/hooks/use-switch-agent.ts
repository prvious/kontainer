import { notify } from "@/components/kit/notify";
import { useAuth } from "@/hooks/use-auth";

/**
 * Sign out toward another agent. Signing out ends every shell and log stream in Rust, and the
 * signed-in shell's signed-out redirect opens Connect with that origin (`nextOrigin`); navigating
 * from here would still see the old session. Resolves false, after saying so, when signing out
 * failed.
 */
export function useSwitchAgent() {
    const { logout } = useAuth();
    return async (origin: string): Promise<boolean> => {
        try {
            await logout(undefined, origin);
        } catch (code) {
            notify.error("Couldn't sign out", { code: String(code), message: "You're still signed in to the current agent." });
            return false;
        }
        return true;
    };
}
