import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { SparkBars } from "@/components/charts/spark-bars";

const start = Date.UTC(2026, 8, 28, 14, 0);
const buckets = [3, null, 7].map((value, index) => ({ start: start + index * 60_000, end: start + (index + 1) * 60_000, value }));

describe("SparkBars", () => {
    it("moves the crosshair with the keyboard and says 'no data' for a gap, never 0", () => {
        render(<SparkBars buckets={buckets} label='Running containers' tooltipRows={(bucket) => [{ label: "Running", value: String(bucket.value) }]} />);
        const chart = screen.getByRole("img", { name: "Running containers" });
        fireEvent.keyDown(chart, { key: "ArrowRight" });
        expect(screen.getByRole("tooltip")).toHaveTextContent("Running3");
        fireEvent.keyDown(chart, { key: "ArrowRight" });
        expect(screen.getByRole("tooltip")).toHaveTextContent("no data");
        expect(screen.getByRole("tooltip")).not.toHaveTextContent(/\b0\b/);
        fireEvent.keyDown(chart, { key: "ArrowRight" });
        expect(screen.getByRole("tooltip")).toHaveTextContent("Running7");
        fireEvent.keyDown(chart, { key: "Escape" });
        expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
    });
});
