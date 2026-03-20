import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import { EnvEditor } from "@/components/env-editor";
import type { EnvVar } from "@/types/service-detail";

function createTestQueryClient() {
    return new QueryClient({
        defaultOptions: {
            queries: { retry: false, gcTime: 0 },
        },
    });
}

const mockAuth: AuthContextValue = {
    user: { id: "1", name: "Test", email: "test@test.com", avatarUrl: "", provider: "github" },
    token: "test-jwt",
    expiresAt: Math.floor(Date.now() / 1000) + 9999,
    agentUrl: "http://localhost:3000",
    isLoading: false,
    login: vi.fn(),
    logout: vi.fn(),
};

function renderEnvEditor(initialEnv: EnvVar[] = []) {
    const queryClient = createTestQueryClient();
    return render(
        <AuthContext value={mockAuth}>
            <QueryClientProvider client={queryClient}>
                <EnvEditor serviceId='svc1' initialEnv={initialEnv} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

describe("EnvEditor", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("renders existing env vars as editable inputs", () => {
        renderEnvEditor([
            { key: "NODE_ENV", value: "production" },
            { key: "PORT", value: "3000" },
        ]);

        expect(screen.getByLabelText("Env key 0")).toHaveValue("NODE_ENV");
        expect(screen.getByLabelText("Env value 0")).toHaveValue("production");
        expect(screen.getByLabelText("Env key 1")).toHaveValue("PORT");
        expect(screen.getByLabelText("Env value 1")).toHaveValue("3000");
    });

    it("shows empty state when no env vars", () => {
        renderEnvEditor([]);

        expect(screen.getByText("No environment variables set")).toBeInTheDocument();
        expect(screen.getByText("Add Variable")).toBeInTheDocument();
    });

    it("adds a new row when clicking Add Variable", async () => {
        const user = userEvent.setup();
        renderEnvEditor([]);

        await user.click(screen.getByText("Add Variable"));

        expect(screen.getByLabelText("Env key 0")).toHaveValue("");
        expect(screen.getByLabelText("Env value 0")).toHaveValue("");
        expect(screen.queryByText("No environment variables set")).not.toBeInTheDocument();
    });

    it("removes a row when clicking the remove button", async () => {
        const user = userEvent.setup();
        renderEnvEditor([
            { key: "FOO", value: "bar" },
            { key: "BAZ", value: "qux" },
        ]);

        await user.click(screen.getByLabelText("Remove env 0"));

        expect(screen.queryByDisplayValue("FOO")).not.toBeInTheDocument();
        expect(screen.getByDisplayValue("BAZ")).toBeInTheDocument();
    });

    it("shows Save Changes button after editing", async () => {
        const user = userEvent.setup();
        renderEnvEditor([{ key: "FOO", value: "bar" }]);

        expect(screen.queryByText("Save Changes")).not.toBeInTheDocument();

        await user.clear(screen.getByLabelText("Env value 0"));
        await user.type(screen.getByLabelText("Env value 0"), "baz");

        expect(screen.getByText("Save Changes")).toBeInTheDocument();
    });

    it("shows Save Changes button after adding a row", async () => {
        const user = userEvent.setup();
        renderEnvEditor([]);

        await user.click(screen.getByText("Add Variable"));

        expect(screen.getByText("Save Changes")).toBeInTheDocument();
    });

    it("shows Save Changes button after removing a row", async () => {
        const user = userEvent.setup();
        renderEnvEditor([{ key: "FOO", value: "bar" }]);

        await user.click(screen.getByLabelText("Remove env 0"));

        expect(screen.getByText("Save Changes")).toBeInTheDocument();
    });

    it("calls update_service_env on save with filtered empty keys", async () => {
        const user = userEvent.setup();
        vi.mocked(invoke).mockResolvedValue({ env: [{ key: "FOO", value: "bar" }] });

        renderEnvEditor([{ key: "FOO", value: "bar" }]);

        // Add an empty row (should be filtered out on save)
        await user.click(screen.getByText("Add Variable"));
        await user.click(screen.getByText("Save Changes"));

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("update_service_env", {
                url: "http://localhost:3000",
                token: "test-jwt",
                serviceId: "svc1",
                env: [{ key: "FOO", value: "bar" }],
            });
        });
    });

    it("calls update_service_env with updated values on save", async () => {
        const user = userEvent.setup();
        vi.mocked(invoke).mockResolvedValue({ env: [{ key: "FOO", value: "updated" }] });

        renderEnvEditor([{ key: "FOO", value: "bar" }]);

        await user.clear(screen.getByLabelText("Env value 0"));
        await user.type(screen.getByLabelText("Env value 0"), "updated");
        await user.click(screen.getByText("Save Changes"));

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("update_service_env", {
                url: "http://localhost:3000",
                token: "test-jwt",
                serviceId: "svc1",
                env: [{ key: "FOO", value: "updated" }],
            });
        });
    });
});
