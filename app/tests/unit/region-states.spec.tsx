import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { EmptyState, ErrorState } from "@/components/kit/region-states";

describe("ErrorState", () => {
    it("shows the code and request id, and COPY DETAILS writes code, request id, and message", async () => {
        const user = userEvent.setup();
        const writeText = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue();
        const onRetry = vi.fn();
        render(
            <ErrorState
                title="Couldn't load containers"
                details={{ status: 503, code: "engine_unavailable", requestId: "01J9ZK4F3Q", message: "The node inventory request failed." }}
                onRetry={onRetry}
            >
                The controller answered, but the node inventory request failed.
            </ErrorState>,
        );
        expect(screen.getByText("503 engine_unavailable")).toBeInTheDocument();
        expect(screen.getByText("req 01J9ZK4F3Q")).toBeInTheDocument();

        await user.click(screen.getByRole("button", { name: "COPY DETAILS" }));
        await waitFor(() =>
            expect(writeText).toHaveBeenCalledWith("code: 503 engine_unavailable\nrequest id: 01J9ZK4F3Q\nmessage: The node inventory request failed."),
        );

        await user.click(screen.getByRole("button", { name: "TRY AGAIN" }));
        expect(onRetry).toHaveBeenCalledTimes(1);
    });
});

describe("EmptyState", () => {
    it("renders title, body, and both actions", async () => {
        const primary = vi.fn();
        render(
            <EmptyState
                title='No containers yet'
                primaryAction={{ label: "CLEAR FILTERS", onClick: primary }}
                secondaryAction={{ label: "SHOW ALL", onClick: vi.fn() }}
            >
                Containers show up here as soon as a stack or service schedules a task.
            </EmptyState>,
        );
        await userEvent.setup().click(screen.getByRole("button", { name: "CLEAR FILTERS" }));
        expect(primary).toHaveBeenCalled();
        expect(screen.getByRole("button", { name: "SHOW ALL" })).toBeInTheDocument();
    });
});
