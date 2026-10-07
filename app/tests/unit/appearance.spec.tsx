import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AppearanceProvider } from "@/contexts/appearance-context";
import { useAppearance } from "@/hooks/use-appearance";

vi.unmock("@/hooks/use-appearance");

function SchemeProbe() {
    const { scheme, resolvedScheme } = useAppearance();
    return (
        <div>
            <span data-testid='scheme'>{scheme}</span>
            <span data-testid='resolved'>{resolvedScheme}</span>
        </div>
    );
}

function renderAppearance() {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    return render(
        <QueryClientProvider client={client}>
            <AppearanceProvider>
                <SchemeProbe />
            </AppearanceProvider>
        </QueryClientProvider>,
    );
}

describe("appearance scheme", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        document.documentElement.classList.remove("dark");
    });

    it.each([
        ["dark", true],
        ["light", false],
    ] as const)("sets the .dark class from a stored %s scheme", async (scheme, dark) => {
        vi.mocked(invoke).mockImplementation(async (command: string) => (command === "load_app_settings" ? { refreshInterval: 5000, scheme } : undefined));
        renderAppearance();
        await waitFor(() => expect(screen.getByTestId("scheme")).toHaveTextContent(scheme));
        await waitFor(() => expect(document.documentElement.classList.contains("dark")).toBe(dark));
    });

    it("follows the OS appearance for the system scheme", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) =>
            command === "load_app_settings" ? { refreshInterval: 5000, scheme: null } : undefined,
        );
        renderAppearance();
        // tests/setup.ts makes the native window report "light".
        await waitFor(() => expect(screen.getByTestId("resolved")).toHaveTextContent("light"));
        expect(screen.getByTestId("scheme")).toHaveTextContent("system");
        expect(document.documentElement).not.toHaveClass("dark");
    });
});
