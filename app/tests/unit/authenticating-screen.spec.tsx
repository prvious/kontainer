import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AuthenticatingScreen } from "@/components/authenticating-screen";

describe("AuthenticatingScreen", () => {
    const defaultProps = {
        provider: "github",
        agentUrl: "https://agent.kontainer.dev",
        step: "browser" as const,
        onCancel: vi.fn(),
    };

    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("renders provider label capitalized", () => {
        render(<AuthenticatingScreen {...defaultProps} provider='github' />);
        expect(screen.getByText("Github Auth")).toBeInTheDocument();
    });

    it("shows current step as active", () => {
        render(<AuthenticatingScreen {...defaultProps} step='auth' />);
        const authStepLabel = screen.getByText("Github Auth");
        expect(authStepLabel).toHaveClass("text-warning");
    });

    it("marks previous steps as completed", () => {
        render(<AuthenticatingScreen {...defaultProps} step='auth' />);
        const browserStepLabel = screen.getByText("Browser Opened");
        expect(browserStepLabel).toHaveClass("text-primary");
    });

    it("cancel button calls onCancel", async () => {
        const onCancel = vi.fn();
        const user = userEvent.setup();
        render(<AuthenticatingScreen {...defaultProps} onCancel={onCancel} />);

        await user.click(screen.getByRole("button", { name: /cancel/i }));

        expect(onCancel).toHaveBeenCalledTimes(1);
    });

    it("renders agent URL in footer", () => {
        render(<AuthenticatingScreen {...defaultProps} agentUrl='https://test.dev' />);
        expect(screen.getByText(/https:\/\/test\.dev/)).toBeInTheDocument();
    });
});
