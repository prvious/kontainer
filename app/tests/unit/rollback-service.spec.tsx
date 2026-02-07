import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import { ServiceDetail } from "@/components/service-detail";
import type { ServiceDetail as ServiceDetailType } from "@/types/service-detail";

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

function renderWithProviders(serviceId: string) {
    const queryClient = createTestQueryClient();

    return render(
        <AuthContext value={mockAuth}>
            <QueryClientProvider client={queryClient}>
                <ServiceDetail serviceId={serviceId} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

const serviceWithPreviousImage: ServiceDetailType = {
    id: "svc1",
    name: "web",
    image: "nginx:2.0",
    stack: "myapp",
    mode: "replicated",
    replicas: 3,
    createdAt: "2025-01-01T00:00:00Z",
    updatedAt: "2025-01-02T00:00:00Z",
    env: [],
    mounts: [],
    constraints: [],
    ports: [],
    updateStatus: null,
    previousImage: "nginx:1.0",
};

const serviceWithoutPreviousImage: ServiceDetailType = {
    id: "svc2",
    name: "api",
    image: "node:20",
    stack: "",
    mode: "replicated",
    replicas: 1,
    createdAt: "2025-01-01T00:00:00Z",
    updatedAt: "2025-01-01T00:00:00Z",
    env: [],
    mounts: [],
    constraints: [],
    ports: [],
    updateStatus: null,
    previousImage: null,
};

describe("Rollback Service", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("shows rollback button when previousImage is present", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "inspect_service") return serviceWithPreviousImage;
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByLabelText("Rollback service")).toBeInTheDocument();
        });
    });

    it("shows previous image in overview", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "inspect_service") return serviceWithPreviousImage;
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByText("Previous Image")).toBeInTheDocument();
        });

        expect(screen.getByText("nginx:1.0")).toBeInTheDocument();
    });

    it("does not show rollback button when no previousImage", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "inspect_service") return serviceWithoutPreviousImage;
            return undefined;
        });

        renderWithProviders("svc2");

        await waitFor(() => {
            expect(screen.getByText("Overview")).toBeInTheDocument();
        });

        expect(screen.queryByLabelText("Rollback service")).not.toBeInTheDocument();
        expect(screen.queryByText("Previous Image")).not.toBeInTheDocument();
    });

    it("clicking rollback calls rollback_service invoke", async () => {
        const user = userEvent.setup();

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "inspect_service") return serviceWithPreviousImage;
            if (cmd === "rollback_service") return { status: "rolling_back" };
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByLabelText("Rollback service")).toBeInTheDocument();
        });

        await user.click(screen.getByLabelText("Rollback service"));

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("rollback_service", {
                url: "http://localhost:8080",
                token: "test-jwt",
                serviceId: "svc1",
            });
        });
    });
});
