import { useQuery } from "@tanstack/react-query";
import { getVersion } from "@tauri-apps/api/app";

/** The desktop build's version from `tauri.conf.json`, or undefined until Tauri answers. */
export function useAppVersion(): string | undefined {
    return useQuery({ queryKey: ["app", "version"], queryFn: getVersion, staleTime: Infinity }).data;
}
