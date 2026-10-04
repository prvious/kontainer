import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { StatusBar } from "@/components/shell/status-bar";

describe("StatusBar", () => {
    it("renders a dash for every missing field", () => {
        render(<StatusBar data={{}} />);
        for (const text of ["Docker —", "CPU —", "MEM —", "— TASKS", "— ALERTS", "— TERMINALS", "— LOG STREAMS", "WS —"]) {
            expect(screen.getByText(text)).toBeInTheDocument();
        }
    });

    it("formats figures and keeps TASKS and ALERTS as neutral dashes until a feature produces them", () => {
        render(
            <StatusBar
                data={{
                    dockerVersion: "24.0.7",
                    cpuPercent: 47.4,
                    memoryUsedBytes: 12.4 * 1024 ** 3,
                    memoryTotalBytes: 24 * 1024 ** 3,
                    terminals: 2,
                    logStreams: 3,
                    connection: "connected",
                }}
            />,
        );
        expect(screen.getByText("Docker 24.0.7")).toBeInTheDocument();
        expect(screen.getByText("CPU 47%")).toBeInTheDocument();
        expect(screen.getByText("MEM 12.4/24G")).toBeInTheDocument();
        expect(screen.getByText("— TASKS")).toBeInTheDocument();
        expect(screen.getByText("— ALERTS")).not.toHaveClass("text-destructive");
        expect(screen.getByText("2 TERMINALS")).toBeInTheDocument();
        expect(screen.getByText("WS CONNECTED")).toBeInTheDocument();
    });

    it("reads RECONNECTING while the controller is away", () => {
        render(<StatusBar data={{ connection: "reconnecting" }} />);
        expect(screen.getByText("RECONNECTING")).toBeInTheDocument();
    });
});
