import type { AccountFieldErrors, FieldProblem, LoginFailure, ProbeView } from "@/types/session";
import { clockTime } from "@/lib/time";

export const DEFAULT_ORIGIN = "http://127.0.0.1:5150";

/** "agent.kontainer.dev" or "localhost:8080" for display. */
export function connectionHost(origin: string): string {
    try {
        return new URL(origin).host;
    } catch {
        return origin;
    }
}
export const PASSWORD_MIN = 12;
export const PASSWORD_MAX = 128;
export const NAME_MAX = 80;

export type ConnectField = "origin" | "displayName" | "email" | "password" | "confirm";
export type ConnectErrors = Partial<Record<ConnectField, string>>;

const EMAIL_SHAPE = /^[^\s@]+@[^\s@.]+(\.[^\s@.]+)+$/;

export function emailError(email: string): string | undefined {
    const value = email.trim();
    if (!value) return "Enter the owner's email.";
    if (!EMAIL_SHAPE.test(value)) return "Enter an email like owner@example.com.";
    return undefined;
}

export function passwordError(password: string): string | undefined {
    const length = [...password].length;
    if (length < PASSWORD_MIN) return `Use at least ${PASSWORD_MIN} characters.`;
    if (length > PASSWORD_MAX) return `Use at most ${PASSWORD_MAX} characters.`;
    return undefined;
}

export function nameError(name: string): string | undefined {
    const value = name.trim();
    if (!value) return "Enter your name.";
    if ([...value].length > NAME_MAX) return `Use at most ${NAME_MAX} characters.`;
    return undefined;
}

export function originError(origin: string): string | undefined {
    try {
        const url = new URL(origin.trim());
        const loopback = ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
        if (url.protocol === "https:" || (url.protocol === "http:" && loopback)) return undefined;
        return "Use https:// (http:// only for localhost).";
    } catch {
        return "Enter the agent URL, e.g. https://agent.example.com.";
    }
}

const SERVER_FIELD_MESSAGES: Record<FieldProblem, (field: string) => string> = {
    invalid: (field) => (field === "email" ? "Enter an email like owner@example.com." : "This value isn't accepted."),
    empty: (field) => (field === "displayName" ? "Enter your name." : "This field is required."),
    too_short: () => `Use at least ${PASSWORD_MIN} characters.`,
    too_long: (field) => (field === "displayName" ? `Use at most ${NAME_MAX} characters.` : `Use at most ${PASSWORD_MAX} characters.`),
};

/** Field errors the controller returned (`validation_failed`), landed on their fields. */
export function serverFieldErrors(fields: AccountFieldErrors | undefined): ConnectErrors {
    const errors: ConnectErrors = {};
    for (const key of ["email", "password", "displayName"] as const) {
        const problem = fields?.[key];
        if (problem) errors[key] = SERVER_FIELD_MESSAGES[problem](key);
    }
    return errors;
}

/** 02.S7: never says which of email or password was wrong. */
export function invalidCredentialsMessage(failure: LoginFailure): string {
    const left = failure.attemptsRemaining;
    if (left === undefined) return "Email or password is incorrect.";
    return `Email or password is incorrect. ${left} ${left === 1 ? "attempt" : "attempts"} left before a 15-minute lock on this email.`;
}

export function lockedMessage(failure: LoginFailure, nowMs: number): string {
    const seconds = failure.retryAfterSeconds ?? 15 * 60;
    const minutes = Math.max(1, Math.ceil(seconds / 60));
    return `Too many failed attempts. Sign-in for this email is locked for ${minutes} more ${minutes === 1 ? "minute" : "minutes"} (until ${clockTime(nowMs + seconds * 1000)}).`;
}

/** The URL field's reason for an unreachable controller (02.S2). */
export function unreachableMessage(origin: string, probe: ProbeView | undefined): string {
    let host = origin;
    let port = "";
    try {
        const url = new URL(origin);
        host = url.hostname;
        port = url.port || (url.protocol === "https:" ? "443" : "80");
    } catch {
        // The raw text is shown as typed.
    }
    switch (probe?.diagnostic) {
        case "dns":
            return `Couldn't resolve ${host}. Check the address.`;
        case "tcp":
            return `Connection refused — nothing is listening on :${port}. Check the port, or the agent's firewall rules.`;
        case "tls":
            return `The TLS handshake with ${host} failed — its certificate isn't trusted or doesn't match the name.`;
        case "http":
            return "The agent answered but isn't ready yet. Check the controller's logs.";
        case "timeout":
            return `${host} didn't answer within 2 seconds.`;
        default:
            return `Couldn't reach ${host}.`;
    }
}

export type DiagnosticStep = { label: "DNS" | "TCP" | "TLS" | "HTTP"; state: "ok" | "failed" | "skipped" };

/** DNS → TCP → TLS (→ HTTP) with the failed step highlighted. */
export function diagnosticSteps(origin: string, probe: ProbeView | undefined): DiagnosticStep[] {
    const tls = origin.trim().startsWith("https:");
    const order: DiagnosticStep["label"][] = tls ? ["DNS", "TCP", "TLS", "HTTP"] : ["DNS", "TCP", "HTTP"];
    const failedAt: Record<string, DiagnosticStep["label"]> = { dns: "DNS", tcp: "TCP", tls: "TLS", http: "HTTP", timeout: "TCP" };
    const failed = probe?.diagnostic ? failedAt[probe.diagnostic] : undefined;
    const failedIndex = failed ? order.indexOf(failed) : -1;
    return order.map((label, index) => ({
        label,
        state: failedIndex === -1 ? "skipped" : index < failedIndex ? "ok" : index === failedIndex ? "failed" : "skipped",
    }));
}
