import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { useNow } from "@/hooks/use-now";

function Clock() {
    return <span data-testid='now'>{useNow()}</span>;
}

describe("useNow", () => {
    afterEach(() => vi.useRealTimers());

    it("ticks once a second from the current time and stops when unmounted", () => {
        vi.useFakeTimers({ now: 1_000_000 });
        const { unmount } = render(<Clock />);
        expect(Number(screen.getByTestId("now").textContent)).toBeGreaterThanOrEqual(1_000_000);
        act(() => vi.advanceTimersByTime(3000));
        expect(screen.getByTestId("now")).toHaveTextContent("1003000");
        unmount();
        expect(vi.getTimerCount()).toBe(0);
    });
});
