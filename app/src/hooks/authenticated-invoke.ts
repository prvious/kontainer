import { invoke } from "@tauri-apps/api/core";
import { inventoryErrorCode, type InventoryErrorCode } from "@/types/inventory";

export type ReportCustodyError = ((profileId: string, code: InventoryErrorCode) => void) | undefined;

/**
 * Invoke a Rust-authenticated controller call with the profile id. Rust validated the controller's
 * answer and returns its typed view, so the result is not checked again (ADR 0032). Failures
 * become closed codes and custody failures are reported to the session so the expired dialog can
 * take over.
 */
export async function authenticatedInvoke<T>(command: string, profileId: string, args: Record<string, unknown>, report: ReportCustodyError): Promise<T> {
    try {
        return await invoke<T>(command, { profileId, ...args });
    } catch (error) {
        const code = inventoryErrorCode(error);
        report?.(profileId, code);
        throw code;
    }
}
