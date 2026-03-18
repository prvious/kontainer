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
    agentUrl: "http://localhost:3000",
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

const replicatedService: ServiceDetailType = {
    id: "svc1",
    name: "web",
    image: "nginx:latest",
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
    previousImage: null,
};

const globalService: ServiceDetailType = {
    id: "svc-global",
    name: "monitoring",
    image: "prometheus:v2",
    stack: "",
    mode: "global",
    replicas: null,
    createdAt: "2025-01-01T00:00:00Z",
    updatedAt: "2025-01-01T00:00:00Z",
    env: [],
    mounts: [],
    constraints: [],
    ports: [],
    updateStatus: null,
    previousImage: null,
};

describe("Restart Service", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("shows restart button for replicated services", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "inspect_service") return replicatedService;
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByLabelText("Restart service")).toBeInTheDocument();
        });
    });

    it("shows restart button for global services", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "inspect_service") return globalService;
            return undefined;
        });

        renderWithProviders("svc-global");

        await waitFor(() => {
            expect(screen.getByLabelText("Restart service")).toBeInTheDocument();
        });
    });

    it("clicking restart calls restart_service invoke", async () => {
        const user = userEvent.setup();

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "inspect_service") return replicatedService;
            if (cmd === "restart_service") return { status: "restarting" };
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByLabelText("Restart service")).toBeInTheDocument();
        });

        await user.click(screen.getByLabelText("Restart service"));

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("restart_service", {
                url: "http://localhost:3000",
                token: "test-jwt",
                serviceId: "svc1",
            });
        });
    });

    it("does not show restart button when loading", () => {
        vi.mocked(invoke).mockImplementation(() => new Promise(() => {}));
        renderWithProviders("svc1");

        expect(screen.queryByLabelText("Restart service")).not.toBeInTheDocument();
    });
});
