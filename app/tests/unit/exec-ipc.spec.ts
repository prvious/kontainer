import { describe, expect, it } from "vitest";
import fixture from "../fixtures/exec-ipc.json";
import { ipcFixture } from "../ipc-fixture";
import { ExecBuffer } from "@/components/exec/exec-buffer";
import { decodeOutput, endedText, type ExecStreamEvent } from "@/types/exec";

// The shared fixture must fit the events Rust's own tests serialize to it.
ipcFixture<ExecStreamEvent[]>(fixture.events);

const output = (text: string): ExecStreamEvent => ({ type: "output", data: btoa(text) });

describe("exec stream IPC contract", () => {
    it("decodes output to the raw bytes, escape sequences included", () => {
        expect(Array.from(decodeOutput("b2sNCg=="))).toEqual([0x6f, 0x6b, 0x0d, 0x0a]);
        expect(decodeOutput(btoa("\u001b[31mred"))[0]).toBe(0x1b);
    });

    it("says why a session ended the way the footer does", () => {
        expect(endedText({ code: 0, reason: "exited" }, null)).toBe("exit 0");
        expect(endedText({ code: 130, reason: "exited" }, null)).toBe("exit 130");
        expect(endedText({ code: null, reason: "exited" }, null)).toBe("exited · exit code unknown");
        expect(endedText({ code: null, reason: "idle_timeout" }, null)).toBe("idle timeout");
        expect(endedText({ code: null, reason: "max_duration" }, null)).toBe("8 hour limit");
        expect(endedText(null, "session_expired")).toBe("session expired");
        expect(endedText(null, "slow_consumer")).toBe("the client fell behind");
        expect(endedText(null, null)).toBe("connection lost");
    });
});

describe("exec buffer", () => {
    it("goes active on start, streams and keeps output, and ends on exit", () => {
        const buffer = new ExecBuffer();
        const seen: string[] = [];
        let notified = 0;
        buffer.subscribe(() => (notified += 1));
        buffer.apply(output("early"), 0);
        const stop = buffer.onOutput((bytes) => seen.push(new TextDecoder().decode(bytes)));
        buffer.apply({ type: "started", shell: "/bin/sh" }, 0);
        buffer.apply(output("ok\r\n"), 0);
        expect(buffer.getSnapshot()).toMatchObject({ status: "active", shell: "/bin/sh" });
        expect(seen).toEqual(["ok\r\n"]);
        expect(buffer.replay().map((bytes) => new TextDecoder().decode(bytes))).toEqual(["early", "ok\r\n"]);

        buffer.apply({ type: "exit", code: 0, reason: "exited" }, 5_000);
        expect(buffer.getSnapshot()).toMatchObject({ status: "ended", exit: { code: 0, reason: "exited" }, endedAt: 5_000 });
        buffer.apply(output("after the end"), 6_000);
        stop();
        expect(seen).toEqual(["ok\r\n"]);
        expect(notified).toBe(2);
    });

    it("fails when the shell never started, and ends when a running one errors or drops", () => {
        const unstarted = new ExecBuffer();
        unstarted.apply({ type: "error", code: "target_not_running", message: "Stopped." }, 1);
        expect(unstarted.getSnapshot()).toMatchObject({ status: "failed", error: { code: "target_not_running" } });

        const dropped = new ExecBuffer();
        dropped.apply({ type: "closed", reason: "connection_lost" }, 1);
        expect(dropped.getSnapshot()).toMatchObject({ status: "failed", closed: "connection_lost", error: { code: "connection_lost" } });

        const running = new ExecBuffer();
        running.apply({ type: "started", shell: "/bin/bash" }, 0);
        running.apply({ type: "closed", reason: "session_expired" }, 2);
        expect(running.getSnapshot()).toMatchObject({ status: "ended", closed: "session_expired", error: null, endedAt: 2 });
    });

    it("keeps at most 2 MiB of output for a terminal that mounts later", () => {
        const buffer = new ExecBuffer();
        buffer.apply({ type: "started", shell: "/bin/sh" }, 0);
        const chunk = btoa("x".repeat(768 * 1024));
        for (let index = 0; index < 4; index += 1) buffer.apply({ type: "output", data: chunk }, 0);
        const kept = buffer.replay().reduce((total, bytes) => total + bytes.length, 0);
        expect(kept).toBeLessThanOrEqual(2 * 1024 * 1024);
        expect(buffer.replay()).toHaveLength(2);
    });
});
