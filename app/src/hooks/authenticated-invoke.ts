import { invoke } from "@tauri-apps/api/core";
import { inventoryErrorCode, type InventoryErrorCode } from "@/types/inventory";

export type ReportCustodyError = ((profileId: string, code: InventoryErrorCode) => void) | undefined;

/**
 * Invoke a Rust-authenticated controller call with the profile id. Failures become closed codes
 * and custody failures are reported to the session so the expired dialog can take over.
 */
export async function authenticatedInvoke<T>(
    command: string,
    profileId: string,
    args: Record<string, unknown>,
    parse: (value: unknown) => T,
    report: ReportCustodyError,
): Promise<T> {
    try {
        return parse(await invoke<unknown>(command, { profileId, ...args }));
    } catch (error) {
        const code = inventoryErrorCode(error);
        report?.(profileId, code);
        throw code;
    }
}
