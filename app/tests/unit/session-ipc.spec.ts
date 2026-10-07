import { describe, expect, it } from "vitest";
import fixture from "../fixtures/session-ipc.json";
import { ipcFixture } from "../ipc-fixture";
import { SESSION_ERROR_CODES, loginFailureFromIpc, type LoginFailure, type ProbeView, type SavedConnection, type SessionView } from "@/types/session";

// The shared fixture must fit the views Rust's own tests serialize to it.
ipcFixture<SessionView>(fixture.sessionView);
ipcFixture<SavedConnection>(fixture.savedConnection);
ipcFixture<ProbeView>(fixture.probeView);
const loginFailure = ipcFixture<LoginFailure>(fixture.loginFailure);
const setupCodeFailure = ipcFixture<LoginFailure>(fixture.setupCodeFailure);

/** Compile-time proof that every `SessionView` field is covered below. */
const SESSION_VIEW_KEYS: Record<keyof SessionView, true> = {
    profileId: true,
    origin: true,
    clusterId: true,
    principalId: true,
    email: true,
    displayName: true,
    label: true,
    accessExpiresAtUnixMs: true,
};

const SAVED_CONNECTION_KEYS: Record<keyof SavedConnection, true> = {
    id: true,
    origin: true,
    label: true,
    lastUsedAtUnixMs: true,
    lastSummary: true,
};

describe("session IPC contract fixture", () => {
    it("matches the exported error codes", () => {
        expect(fixture.errorCodes).toEqual([...SESSION_ERROR_CODES]);
    });

    it("carries no field the session view and saved connection lack", () => {
        expect(Object.keys(fixture.sessionView).sort()).toEqual(Object.keys(SESSION_VIEW_KEYS).sort());
        expect(Object.keys(fixture.savedConnection).sort()).toEqual(Object.keys(SAVED_CONNECTION_KEYS).sort());
    });

    it("reads login failures the way Rust writes them and closes anything else over a code", () => {
        expect(loginFailureFromIpc(loginFailure)).toEqual(loginFailure);
        expect(loginFailureFromIpc(setupCodeFailure)).toEqual(setupCodeFailure);
        // Plain SessionError strings still arrive from the non-login commands.
        expect(loginFailureFromIpc("profile_conflict")).toEqual({ code: "profile_conflict" });
        expect(loginFailureFromIpc({ code: "not-a-code" })).toEqual({ code: "invalid_response" });
        expect(loginFailureFromIpc(new Error("native-secret-canary"))).toEqual({ code: "invalid_response" });
    });

    it("lists the registered custody commands", () => {
        expect(fixture.commands).toEqual([
            "session_bootstrap",
            "session_login",
            "session_register",
            "session_restore",
            "session_retry_commit",
            "session_logout",
            "session_connections",
            "session_forget_connection",
            "connection_probe",
            "controller_health",
        ]);
    });
});
