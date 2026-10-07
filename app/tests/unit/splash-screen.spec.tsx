import { describe, expect, it, vi, afterEach } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RESTORE_TIMEOUT_MS, SplashScreen } from "@/components/splash-screen";

function renderSplash(props: Parameters<typeof SplashScreen>[0] = {}) {
    return render(
        <QueryClientProvider client={new QueryClient()}>
            <SplashScreen {...props} />
        </QueryClientProvider>,
    );
}

describe("SplashScreen", () => {
    afterEach(() => vi.useRealTimers());

    it("shows progress while restoring, then the startup failure after 15 s without an answer", () => {
        vi.useFakeTimers();
        const onRetry = vi.fn();
        renderSplash({ onRetry });
        expect(screen.getByText("Initializing runtime…")).toBeInTheDocument();
        expect(screen.queryByRole("alert")).not.toBeInTheDocument();

        act(() => vi.advanceTimersByTime(RESTORE_TIMEOUT_MS));
        expect(screen.getByRole("alert")).toHaveTextContent("Couldn't start the local runtime");
        expect(screen.getByText("restore_timeout")).toBeInTheDocument();
        act(() => screen.getByRole("button", { name: "RETRY" }).click());
        expect(onRetry).toHaveBeenCalledTimes(1);
    });

    it("shows a restore failure at once, with OPEN LOGS unavailable", () => {
        renderSplash({ failure: "vault_unavailable" });
        expect(screen.getByText("Stopped at: unlocking credential store")).toBeInTheDocument();
        expect(screen.getByText("vault_unavailable")).toBeInTheDocument();
        expect(screen.getByRole("button", { name: /OPEN LOGS/ })).toHaveAttribute("aria-disabled", "true");
    });
});
