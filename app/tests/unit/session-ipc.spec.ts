import { describe, expect, it } from "vitest";
import fixtureSource from "../fixtures/session-ipc.json?raw";
import {
    SESSION_ERROR_CODES,
    loginFailureFromIpc,
    probeViewFromIpc,
    savedConnectionsFromIpc,
    sessionViewFromIpc,
    type LoginFailure,
    type ProbeView,
    type SavedConnection,
    type SessionView,
} from "@/types/session";

interface SessionIpcFixture {
    commands: string[];
    sessionView: SessionView;
    savedConnection: SavedConnection;
    probeView: ProbeView;
    loginFailure: LoginFailure;
    errorCodes: string[];
}

const fixture: SessionIpcFixture = JSON.parse(fixtureSource) as SessionIpcFixture;

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

    it("matches the session view and saved connection shapes", () => {
        expect(Object.keys(fixture.sessionView).sort()).toEqual(Object.keys(SESSION_VIEW_KEYS).sort());
        expect(sessionViewFromIpc(fixture.sessionView)).toEqual(fixture.sessionView);
        expect(Object.keys(fixture.savedConnection).sort()).toEqual(Object.keys(SAVED_CONNECTION_KEYS).sort());
        expect(savedConnectionsFromIpc([fixture.savedConnection])).toEqual([fixture.savedConnection]);
    });

    it("reads probe views and login failures the way Rust writes them", () => {
        expect(probeViewFromIpc(fixture.probeView)).toEqual(fixture.probeView);
        expect(probeViewFromIpc({ status: "ready" })).toEqual({ status: "ready" });
        expect(loginFailureFromIpc(fixture.loginFailure)).toEqual(fixture.loginFailure);
        // Plain SessionError strings still arrive from the non-login commands.
        expect(loginFailureFromIpc("profile_conflict")).toEqual({ code: "profile_conflict" });
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

    it("drops unexpected native fields before they can enter React state", () => {
        expect(sessionViewFromIpc({ ...fixture.sessionView, accessToken: "credential-canary", refreshToken: "refresh-canary" })).toEqual(fixture.sessionView);
        expect(loginFailureFromIpc({ ...fixture.loginFailure, password: "secret-canary" })).toEqual(fixture.loginFailure);
    });

    it("rejects malformed native output with a closed code", () => {
        expect(() => sessionViewFromIpc({ ...fixture.sessionView, profileId: null })).toThrow("invalid_response");
        expect(() => savedConnectionsFromIpc([{ ...fixture.savedConnection, lastSummary: { nodes: -1, services: 0 } }])).toThrow("invalid_response");
        expect(() => probeViewFromIpc({ status: "maybe" })).toThrow("invalid_response");
        expect(loginFailureFromIpc({ code: "not-a-code" })).toEqual({ code: "invalid_response" });
    });
});
