import { useCallback, useEffect, useRef, useState } from "react";
import { useAuth } from "@/hooks/use-auth";

export interface LogEntry {
    id: number;
    stream: "stdout" | "stderr";
    timestamp: string;
    line: string;
}

export interface LogOptions {
    tail: string;
    timestamps: boolean;
    stdout: boolean;
    stderr: boolean;
}

export type ConnectionStatus = "disconnected" | "connecting" | "connected" | "error";

const MAX_LOG_LINES = 5000;
const MAX_BACKOFF = 16_000;

let nextLogId = 0;

export function useLogStream(path: string, options: LogOptions) {
    const { agentUrl, token } = useAuth();
    const [logs, setLogs] = useState<LogEntry[]>([]);
    const [status, setStatus] = useState<ConnectionStatus>("disconnected");
    const [isPaused, setIsPaused] = useState(false);

    const wsRef = useRef<WebSocket | null>(null);
    const pauseBufferRef = useRef<LogEntry[]>([]);
    const isPausedRef = useRef(false);
    const backoffRef = useRef(1000);
    const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const unmountedRef = useRef(false);

    // Store latest values in refs so the WebSocket callbacks always use current values
    const agentUrlRef = useRef(agentUrl);
    const tokenRef = useRef(token);
    agentUrlRef.current = agentUrl;
    tokenRef.current = token;

    const appendLogs = useCallback((entries: LogEntry[]) => {
        setLogs((prev) => {
            const merged = [...prev, ...entries];
            if (merged.length > MAX_LOG_LINES) {
                return merged.slice(merged.length - MAX_LOG_LINES);
            }
            return merged;
        });
    }, []);

    const clearLogs = useCallback(() => {
        setLogs([]);
        pauseBufferRef.current = [];
    }, []);

    const pause = useCallback(() => {
        setIsPaused(true);
        isPausedRef.current = true;
    }, []);

    const resume = useCallback(() => {
        setIsPaused(false);
        isPausedRef.current = false;
        if (pauseBufferRef.current.length > 0) {
            appendLogs(pauseBufferRef.current);
            pauseBufferRef.current = [];
        }
    }, [appendLogs]);

    // Store options in a ref so reconnect always uses latest values
    const optionsRef = useRef(options);
    optionsRef.current = options;

    const connect = useCallback(() => {
        const url = agentUrlRef.current;
        const tok = tokenRef.current;
        if (!url || !tok) return;

        // Clean up previous connection
        if (wsRef.current) {
            wsRef.current.close();
            wsRef.current = null;
        }

        setStatus("connecting");

        const opts = optionsRef.current;
        const wsUrl =
            url.replace(/^http/, "ws") +
            path +
            `?token=${encodeURIComponent(tok)}` +
            `&tail=${opts.tail}` +
            `&timestamps=${opts.timestamps}` +
            `&stdout=${opts.stdout}` +
            `&stderr=${opts.stderr}`;

        const ws = new WebSocket(wsUrl);
        wsRef.current = ws;

        ws.onopen = () => {
            if (unmountedRef.current) return;
            setStatus("connected");
            backoffRef.current = 1000;
        };

        ws.onmessage = (ev) => {
            if (unmountedRef.current) return;
            try {
                const data = JSON.parse(ev.data);
                if (data.error) {
                    console.warn("[useLogStream] server error:", data.error);
                    setStatus("error");
                    return;
                }
                const entry: LogEntry = {
                    id: nextLogId++,
                    stream: data.stream === "stderr" ? "stderr" : "stdout",
                    timestamp: data.timestamp ?? "",
                    line: typeof data.line === "string" ? data.line : String(data.line ?? ""),
                };
                if (isPausedRef.current) {
                    pauseBufferRef.current.push(entry);
                } else {
                    appendLogs([entry]);
                }
            } catch (err) {
                console.warn("[useLogStream] failed to parse WebSocket message:", err);
            }
        };

        ws.onclose = () => {
            if (unmountedRef.current) return;
            setStatus("disconnected");
            const delay = backoffRef.current;
            backoffRef.current = Math.min(backoffRef.current * 2, MAX_BACKOFF);
            reconnectTimerRef.current = setTimeout(() => {
                if (!unmountedRef.current) {
                    connectRef.current();
                }
            }, delay);
        };

        ws.onerror = () => {
            if (unmountedRef.current) return;
            setStatus("error");
        };
    }, [path, appendLogs]);

    const connectRef = useRef(connect);
    connectRef.current = connect;

    const reconnect = useCallback(() => {
        if (reconnectTimerRef.current) {
            clearTimeout(reconnectTimerRef.current);
            reconnectTimerRef.current = null;
        }
        backoffRef.current = 1000;
        clearLogs();
        connectRef.current();
    }, [clearLogs]);

    // Connect when path changes. agentUrl/token/options are read from refs
    // so the effect doesn't need to re-run when they change.
    useEffect(() => {
        unmountedRef.current = false;
        connectRef.current();

        return () => {
            unmountedRef.current = true;
            if (reconnectTimerRef.current) {
                clearTimeout(reconnectTimerRef.current);
                reconnectTimerRef.current = null;
            }
            if (wsRef.current) {
                wsRef.current.close();
                wsRef.current = null;
            }
        };
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [path]);

    return { logs, status, isPaused, clearLogs, pause, resume, reconnect };
}
