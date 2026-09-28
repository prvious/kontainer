import { invoke } from "@tauri-apps/api/core";
import { sessionErrorCode } from "@/types/session";

export async function invokeSession<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    try {
        return await invoke<T>(command, args);
    } catch (error) {
        throw sessionErrorCode(error);
    }
}
