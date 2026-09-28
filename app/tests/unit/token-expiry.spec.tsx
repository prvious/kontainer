import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { AuthProvider } from "@/contexts/auth-context";
import { useAuth } from "@/hooks/use-auth";
import type { SessionView } from "@/types/session";

const session: SessionView = {
    profileId: "local-profile-id",
    origin: "https://controller.example/",
    clusterId: "cluster-id",
    principalId: "principal-id",
    githubLogin: "jane",
    label: "Production",
    accessExpiresAtUnixMs: 2_000_000_000_000,
};

function TestConsumer() {
    const { session: active, isLoading } = useAuth();
    return (
        <div>
            <span data-testid='profile'>{active?.profileId ?? "none"}</span>
            <span data-testid='loading'>{String(isLoading)}</span>
        </div>
    );
}

describe("Session Restore Authority", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("shows no session when Rust rejects expired credentials", async () => {
        vi.mocked(invoke).mockResolvedValue(null);
        render(
            <AuthProvider>
                <TestConsumer />
            </AuthProvider>,
        );
        await waitFor(() => expect(screen.getByTestId("loading")).toHaveTextContent("false"));
        expect(screen.getByTestId("profile")).toHaveTextContent("none");
        expect(invoke).toHaveBeenCalledWith("session_restore", undefined);
    });

    it("shows the non-secret session view when Rust restores valid credentials", async () => {
        vi.mocked(invoke).mockResolvedValue(session);
        render(
            <AuthProvider>
                <TestConsumer />
            </AuthProvider>,
        );
        await waitFor(() => expect(screen.getByTestId("profile")).toHaveTextContent("local-profile-id"));
    });
});
