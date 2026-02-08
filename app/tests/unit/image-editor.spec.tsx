import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import { ImageEditor } from "@/components/image-editor";

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
    agentUrl: "http://localhost:8080",
    isLoading: false,
    login: vi.fn(),
    logout: vi.fn(),
};

function renderImageEditor(currentImage = "nginx:1.24") {
    const queryClient = createTestQueryClient();
    return render(
        <AuthContext value={mockAuth}>
            <QueryClientProvider client={queryClient}>
                <ImageEditor serviceId='svc1' currentImage={currentImage} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

describe("ImageEditor", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("displays current image with edit button", () => {
        renderImageEditor("nginx:1.24");

        expect(screen.getByText("nginx:1.24")).toBeInTheDocument();
        expect(screen.getByLabelText("Edit image")).toBeInTheDocument();
    });

    it("shows input when edit button is clicked", async () => {
        const user = userEvent.setup();
        renderImageEditor("nginx:1.24");

        await user.click(screen.getByLabelText("Edit image"));

        expect(screen.getByLabelText("Image tag")).toHaveValue("nginx:1.24");
        expect(screen.getByLabelText("Save image")).toBeInTheDocument();
        expect(screen.getByLabelText("Cancel edit")).toBeInTheDocument();
    });

    it("cancels editing and restores original value", async () => {
        const user = userEvent.setup();
        renderImageEditor("nginx:1.24");

        await user.click(screen.getByLabelText("Edit image"));
        await user.clear(screen.getByLabelText("Image tag"));
        await user.type(screen.getByLabelText("Image tag"), "nginx:1.25");
        await user.click(screen.getByLabelText("Cancel edit"));

        expect(screen.getByText("nginx:1.24")).toBeInTheDocument();
        expect(screen.queryByLabelText("Image tag")).not.toBeInTheDocument();
    });

    it("calls update_service_image on save", async () => {
        const user = userEvent.setup();
        vi.mocked(invoke).mockResolvedValue({ image: "nginx:1.25" });

        renderImageEditor("nginx:1.24");

        await user.click(screen.getByLabelText("Edit image"));
        await user.clear(screen.getByLabelText("Image tag"));
        await user.type(screen.getByLabelText("Image tag"), "nginx:1.25");
        await user.click(screen.getByLabelText("Save image"));

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("update_service_image", {
                url: "http://localhost:8080",
                token: "test-jwt",
                serviceId: "svc1",
                image: "nginx:1.25",
            });
        });
    });

    it("saves on Enter key", async () => {
        const user = userEvent.setup();
        vi.mocked(invoke).mockResolvedValue({ image: "nginx:1.25" });

        renderImageEditor("nginx:1.24");

        await user.click(screen.getByLabelText("Edit image"));
        await user.clear(screen.getByLabelText("Image tag"));
        await user.type(screen.getByLabelText("Image tag"), "nginx:1.25{Enter}");

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("update_service_image", {
                url: "http://localhost:8080",
                token: "test-jwt",
                serviceId: "svc1",
                image: "nginx:1.25",
            });
        });
    });

    it("cancels on Escape key", async () => {
        const user = userEvent.setup();
        renderImageEditor("nginx:1.24");

        await user.click(screen.getByLabelText("Edit image"));
        await user.clear(screen.getByLabelText("Image tag"));
        await user.type(screen.getByLabelText("Image tag"), "nginx:1.25");
        await user.keyboard("{Escape}");

        expect(screen.getByText("nginx:1.24")).toBeInTheDocument();
        expect(screen.queryByLabelText("Image tag")).not.toBeInTheDocument();
    });

    it("does not call invoke when saving unchanged image", async () => {
        const user = userEvent.setup();
        renderImageEditor("nginx:1.24");

        await user.click(screen.getByLabelText("Edit image"));
        await user.click(screen.getByLabelText("Save image"));

        expect(invoke).not.toHaveBeenCalled();
        expect(screen.getByText("nginx:1.24")).toBeInTheDocument();
    });

    it("does not call invoke when saving empty image", async () => {
        const user = userEvent.setup();
        renderImageEditor("nginx:1.24");

        await user.click(screen.getByLabelText("Edit image"));
        await user.clear(screen.getByLabelText("Image tag"));
        await user.click(screen.getByLabelText("Save image"));

        expect(invoke).not.toHaveBeenCalled();
    });
});
