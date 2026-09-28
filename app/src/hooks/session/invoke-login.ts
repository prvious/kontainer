import { invoke } from "@tauri-apps/api/core";
import { loginFailureFromIpc } from "@/types/session";

/** Invoke a sign-in, registration, or bootstrap command; failures become `LoginFailure`s. */
export async function invokeLogin<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    try {
        return await invoke<T>(command, args);
    } catch (error) {
        throw loginFailureFromIpc(error);
    }
}
