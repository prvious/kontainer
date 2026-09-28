import { describe, expect, it } from "vitest";
import fixtureSource from "../fixtures/session-ipc.json?raw";
import { SESSION_ERROR_CODES, loginHintFromIpc, sessionViewFromIpc, type LoginHint, type SessionView } from "@/types/session";

interface SessionIpcFixture {
    commands: string[];
    sessionView: SessionView;
    loginHint: LoginHint;
    errorCodes: string[];
}

const fixture: SessionIpcFixture = JSON.parse(fixtureSource) as SessionIpcFixture;

/** Compile-time proof that every `SessionView` field is covered below. */
const SESSION_VIEW_KEYS: Record<keyof SessionView, true> = {
    profileId: true,
    origin: true,
    clusterId: true,
    principalId: true,
    githubLogin: true,
    label: true,
    accessExpiresAtUnixMs: true,
};

/** Compile-time proof that every `LoginHint` field is covered below. */
const LOGIN_HINT_KEYS: Record<keyof LoginHint, true> = {
    origin: true,
    label: true,
};

describe("session IPC contract fixture", () => {
    it("matches the exported error codes", () => {
        expect(fixture.errorCodes).toEqual([...SESSION_ERROR_CODES]);
    });

    it("matches the session view shape", () => {
        expect(Object.keys(fixture.sessionView).sort()).toEqual(Object.keys(SESSION_VIEW_KEYS).sort());
        expect(fixture.sessionView.profileId).toBeTypeOf("string");
        expect(fixture.sessionView.accessExpiresAtUnixMs).toBeTypeOf("number");
    });

    it("matches the login hint shape", () => {
        expect(Object.keys(fixture.loginHint).sort()).toEqual(Object.keys(LOGIN_HINT_KEYS).sort());
    });

    it("lists the registered custody commands", () => {
        expect(fixture.commands).toEqual([
            "session_login",
            "session_cancel_login",
            "session_restore",
            "session_retry_commit",
            "session_logout",
            "session_login_hint",
        ]);
    });

    it("drops unexpected native fields before they can enter React state", () => {
        expect(sessionViewFromIpc({ ...fixture.sessionView, accessToken: "credential-canary", refreshToken: "refresh-canary" })).toEqual(fixture.sessionView);
        expect(loginHintFromIpc({ ...fixture.loginHint, verifier: "verifier-canary" })).toEqual(fixture.loginHint);
    });

    it("rejects malformed native output with a closed code", () => {
        expect(() => sessionViewFromIpc({ ...fixture.sessionView, profileId: null })).toThrow("invalid_response");
        expect(() => loginHintFromIpc({ ...fixture.loginHint, label: 123 })).toThrow("invalid_response");
    });
});
