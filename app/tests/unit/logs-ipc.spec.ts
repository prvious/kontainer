import { describe, expect, it } from "vitest";
import fixture from "../fixtures/logs-ipc.json";
import { LogBuffer } from "@/components/logs/log-buffer";
import { lineMatcher, logLevel, logStreamEventFromIpc, type LogLine } from "@/types/logs";

const [linesEvent] = fixture.events;
const line = (seq: number, text: string, stream: LogLine["stream"] = "stdout"): LogLine => ({
    ...(linesEvent as { lines: LogLine[] }).lines[0],
    seq,
    ts: 1_790_000_000_000 + seq,
    text,
    stream,
});

describe("log stream IPC contract", () => {
    it("parses every event exactly as Rust serializes it and refuses anything else", () => {
        for (const event of fixture.events) expect(logStreamEventFromIpc(event)).toEqual(event);
        expect(() => logStreamEventFromIpc({ type: "gap", dropped: 1, secret: "canary" })).toThrow();
        expect(() => logStreamEventFromIpc({ type: "gap", dropped: -1 })).toThrow();
        expect(() => logStreamEventFromIpc({ type: "ended", reason: "deleted", code: "x", message: "" })).toThrow();
        expect(() => logStreamEventFromIpc({ type: "lines", lines: [{ ...line(1, "a"), stream: "stdin" }] })).toThrow();
        expect(() => logStreamEventFromIpc({ type: "unknown" })).toThrow();
        expect(() => logStreamEventFromIpc(null)).toThrow();
    });

    it("parses levels from the line, gives stderr without one ERR, and stdout without one nothing", () => {
        const cases: [string, LogLine["stream"], string | null, string | null][] = [
            ["level=info msg=started", "stdout", "INFO", "neutral"],
            ["[WARNING] disk almost full", "stdout", "WARN", "warning"],
            ["Warn: slow query", "stdout", "WARN", "warning"],
            ["ERROR connection refused", "stdout", "ERROR", "destructive"],
            ["fatal: out of memory", "stderr", "FATAL", "destructive"],
            ["debug tick", "stdout", "DEBUG", "neutral"],
            ["TRACE enter handler", "stdout", "TRACE", "neutral"],
            ["INFORMATION is not a level", "stdout", null, null],
            ["panic: nil map", "stderr", "ERR", "destructive"],
            ["GET / 200", "stdout", null, null],
        ];
        for (const [text, stream, label, tone] of cases) {
            const level = logLevel({ text, stream });
            expect(level?.label ?? null, text).toBe(label);
            expect(level?.tone ?? null, text).toBe(tone);
        }
    });

    it("filters by case-insensitive substring, by regex between slashes, and literally when the regex is invalid", () => {
        expect(lineMatcher("")("anything")).toBe(true);
        expect(lineMatcher("TIMEOUT")("upstream timeout (10s)")).toBe(true);
        expect(lineMatcher("timeout")("GET / 200")).toBe(false);
        expect(lineMatcher("/50[24]/")("502 Bad Gateway")).toBe(true);
        expect(lineMatcher("/50[24]/")("503 Unavailable")).toBe(false);
        expect(lineMatcher("/([/")("a /([/ b")).toBe(true);
    });
});

describe("log buffer", () => {
    it("appends lines and gaps, caps at the scrollback, and tracks the window", () => {
        const buffer = new LogBuffer(3);
        buffer.opened();
        buffer.apply({ type: "lines", lines: [line(1, "a"), line(2, "b")] }, 0);
        buffer.apply({ type: "gap", dropped: 9 }, 0);
        buffer.apply({ type: "lines", lines: [line(3, "c"), line(4, "d")] }, 0);
        const snapshot = buffer.getSnapshot();
        expect(snapshot.rows.map((row) => (row.kind === "line" ? row.line.text : `gap ${row.dropped}`))).toEqual(["gap 9", "c", "d"]);
        expect(snapshot.received).toBe(4);
        expect(snapshot.firstTs).toBe(line(3, "").ts);
        expect(snapshot.lastTs).toBe(line(4, "").ts);
        expect(snapshot.status).toBe("live");
    });

    it("moves through reconnecting, reconnected, and ended, and restarts with or without its lines", () => {
        const buffer = new LogBuffer(100);
        let notified = 0;
        buffer.subscribe(() => (notified += 1));
        buffer.apply({ type: "lines", lines: [line(1, "a")] }, 0);
        buffer.apply({ type: "reconnecting", attempt: 2, nextRetryMs: 2_000 }, 10_000);
        expect(buffer.getSnapshot()).toMatchObject({ status: "reconnecting", reconnect: { attempt: 2, retryAt: 12_000 }, droppedAt: 10_000 });
        buffer.apply({ type: "reconnecting", attempt: 3, nextRetryMs: 4_000 }, 12_000);
        expect(buffer.getSnapshot().droppedAt).toBe(10_000);
        buffer.apply({ type: "reconnected" }, 13_000);
        expect(buffer.getSnapshot()).toMatchObject({ status: "live", reconnect: null, droppedAt: null });
        buffer.apply({ type: "ended", reason: "service_removed", code: "service_removed", message: "gone" }, 14_000);
        expect(buffer.getSnapshot().status).toBe("ended");
        expect(notified).toBe(5);

        buffer.restart(true);
        expect(buffer.getSnapshot()).toMatchObject({ status: "connecting", ended: null });
        buffer.apply({ type: "lines", lines: [line(1, "again")] }, 15_000);
        const keys = buffer.getSnapshot().rows.map((row) => row.key);
        expect(new Set(keys).size, "a restarted stream's seq 1 does not collide").toBe(2);
        buffer.restart(false);
        expect(buffer.getSnapshot()).toMatchObject({ rows: [], received: 2, status: "connecting" });
    });
});
