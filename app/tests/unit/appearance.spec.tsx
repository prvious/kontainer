import { describe, expect, it, vi, beforeEach } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AppearanceProvider } from "@/contexts/appearance-context";
import { useAppearance } from "@/hooks/use-appearance";

vi.unmock("@/hooks/use-appearance");

function SchemeControls() {
    const { scheme, resolvedScheme, setScheme } = useAppearance();
    return (
        <div>
            <span data-testid='scheme'>{scheme}</span>
            <span data-testid='resolved'>{resolvedScheme}</span>
            <button onClick={() => setScheme("light")}>Light</button>
            <button onClick={() => setScheme("dark")}>Dark</button>
        </div>
    );
}

function renderAppearance() {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    return render(
        <QueryClientProvider client={client}>
            <AppearanceProvider>
                <SchemeControls />
            </AppearanceProvider>
        </QueryClientProvider>,
    );
}

describe("appearance scheme", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        document.documentElement.classList.remove("dark");
    });

    it("toggles the .dark class from the resolved scheme and persists the choice", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) =>
            command === "load_app_settings" ? { refreshInterval: 5000, scheme: "dark" } : undefined,
        );
        renderAppearance();
        await waitFor(() => expect(screen.getByTestId("scheme")).toHaveTextContent("dark"));
        expect(document.documentElement).toHaveClass("dark");

        await act(async () => userEvent.setup().click(screen.getByText("Light")));
        await waitFor(() => expect(document.documentElement).not.toHaveClass("dark"));
        expect(invoke).toHaveBeenCalledWith("save_app_settings", { settings: expect.objectContaining({ scheme: "light", refreshInterval: 5000 }) });

        await act(async () => userEvent.setup().click(screen.getByText("Dark")));
        await waitFor(() => expect(document.documentElement).toHaveClass("dark"));
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
