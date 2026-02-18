import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { AuthProvider } from "@/contexts/auth-context";
import { useAuth } from "@/hooks/use-auth";
import type { StoredProfiles } from "@/types/connection";

function TestConsumer() {
    const { user, isLoading } = useAuth();
    return (
        <div>
            <span data-testid="user">{user ? user.name : "none"}</span>
            <span data-testid="isLoading">{isLoading ? "true" : "false"}</span>
        </div>
    );
}

describe("Token Expiry on Mount", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    function renderWithProvider() {
        return render(
            <AuthProvider>
                <TestConsumer />
            </AuthProvider>,
        );
    }

    it("does not restore a session with an expired token on mount", async () => {
        const nowSec = Math.floor(Date.now() / 1000);
        const expiredProfiles: StoredProfiles = {
            profiles: [
                {
                    id: "p1",
                    label: "Default",
                    agentUrl: "http://localhost:8080",
                    userId: "123",
                    userName: "Jane Doe",
                    userEmail: "jane@example.com",
                    userAvatarUrl: "https://example.com/avatar.png",
                    userProvider: "github",
                    token: "expired.jwt.token",
                    expiresAt: nowSec - 60,
                },
            ],
            activeProfileId: "p1",
        };

        const emptyProfiles: StoredProfiles = {
            profiles: [],
            activeProfileId: null,
        };

        let deleted = false;
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return deleted ? emptyProfiles : expiredProfiles;
            if (cmd === "delete_profile") {
                deleted = true;
                return undefined;
            }
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        expect(screen.getByTestId("user")).toHaveTextContent("none");
        expect(invoke).toHaveBeenCalledWith("delete_profile", { profileId: "p1" });
    });

    it("restores a session with a valid token on mount", async () => {
        const nowSec = Math.floor(Date.now() / 1000);
        const validProfiles: StoredProfiles = {
            profiles: [
                {
                    id: "p1",
                    label: "Default",
                    agentUrl: "http://localhost:8080",
                    userId: "123",
                    userName: "Jane Doe",
                    userEmail: "jane@example.com",
                    userAvatarUrl: "https://example.com/avatar.png",
                    userProvider: "github",
                    token: "valid.jwt.token",
                    expiresAt: nowSec + 99999,
                },
            ],
            activeProfileId: "p1",
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return validProfiles;
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        expect(screen.getByTestId("user")).toHaveTextContent("Jane Doe");
    });
});
