import { useAuth } from "@/hooks/use-auth";

/** Controller queries pause while signed out or while the session-expired dialog is up (02.S5). */
export function useQueriesPaused(): boolean {
    const { session, expired } = useAuth();
    return !session || expired;
}
