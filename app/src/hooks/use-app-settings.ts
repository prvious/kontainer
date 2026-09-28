import { invoke } from "@tauri-apps/api/core";
import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { DEFAULT_APP_SETTINGS, type AppSettings } from "@/types/settings";

/** App-level (not profile-scoped) data lives under `["app", ...]` and survives sign-in changes. */
export const appSettingsQuery = queryOptions({
    queryKey: ["app", "settings"] as const,
    queryFn: async () => ({ ...DEFAULT_APP_SETTINGS, ...(await invoke<Partial<AppSettings>>("load_app_settings")) }),
    staleTime: Infinity,
});

export function useAppSettings() {
    return useQuery(appSettingsQuery);
}

/** Writes the whole settings document; the cache updates first so the UI applies the change at once. */
export function useSaveAppSettings() {
    const queryClient = useQueryClient();
    return useMutation({
        mutationFn: (settings: AppSettings) => invoke<void>("save_app_settings", { settings }),
        onMutate: async (settings) => {
            await queryClient.cancelQueries({ queryKey: appSettingsQuery.queryKey });
            const previous = queryClient.getQueryData(appSettingsQuery.queryKey);
            queryClient.setQueryData(appSettingsQuery.queryKey, settings);
            return { previous };
        },
        onError: (_error, _settings, context) => {
            if (context?.previous) queryClient.setQueryData(appSettingsQuery.queryKey, context.previous);
        },
    });
}
