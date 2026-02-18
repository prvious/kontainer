import { QueryCache, QueryClient } from "@tanstack/react-query";

type AuthErrorHandler = () => void;
let onAuthError: AuthErrorHandler | null = null;

export function setAuthErrorHandler(handler: AuthErrorHandler | null) {
    onAuthError = handler;
}

export const queryClient = new QueryClient({
    queryCache: new QueryCache({
        onError: (error) => {
            if (String(error).includes("401") && onAuthError) {
                onAuthError();
            }
        },
    }),
    defaultOptions: {
        queries: {
            retry: (failureCount, error) => {
                if (String(error).includes("401")) return false;
                return failureCount < 5;
            },
        },
    },
});
