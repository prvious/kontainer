import { useCallback, useRef, useState } from "react";
import { useAuth } from "@/hooks/use-auth";
import type { Terminal } from "ghostty-web";

export type ExecStatus = "disconnected" | "connecting" | "connected" | "error" | "exited";

// Text-frame control messages from server
interface ExecControlMsg {
    type: "error" | "exit";
    message?: string;
    code?: number;
}

const encoder = new TextEncoder();

export function useExecStream(containerId: string, nodeId: string, shell?: string, user?: string) {
    const { agentUrl, token } = useAuth();
    const [status, setStatus] = useState<ExecStatus>("disconnected");
    const [exitCode, setExitCode] = useState<number | null>(null);

    const wsRef = useRef<WebSocket | null>(null);
    const terminalRef = useRef<Terminal | null>(null);

    const sendInput = useCallback((data: string) => {
        const ws = wsRef.current;
        if (ws?.readyState === WebSocket.OPEN) {
            // Send terminal input as binary frame
            ws.send(encoder.encode(data));
        }
    }, []);

    const sendResize = useCallback((cols: number, rows: number) => {
        const ws = wsRef.current;
        if (ws?.readyState === WebSocket.OPEN) {
            // Send resize as JSON text frame
            ws.send(JSON.stringify({ type: "resize", cols, rows }));
        }
    }, []);

    const attachTerminal = useCallback((terminal: Terminal) => {
        terminalRef.current = terminal;
    }, []);

    const connect = useCallback(
        (initialCols = 80, initialRows = 24) => {
            if (!agentUrl || !token) return;

            // Clean up previous
            if (wsRef.current) {
                wsRef.current.close();
                wsRef.current = null;
            }

            setStatus("connecting");
            setExitCode(null);

            let wsUrl =
                agentUrl.replace(/^http/, "ws") +
                `/api/exec/${encodeURIComponent(containerId)}` +
                `?token=${encodeURIComponent(token)}` +
                `&nodeID=${encodeURIComponent(nodeId)}` +
                `&cols=${initialCols}` +
                `&rows=${initialRows}`;
            if (shell) {
                wsUrl += `&shell=${encodeURIComponent(shell)}`;
            }
            if (user) {
                wsUrl += `&user=${encodeURIComponent(user)}`;
            }

            const ws = new WebSocket(wsUrl);
            ws.binaryType = "arraybuffer";
            wsRef.current = ws;

            let didOpen = false;

            ws.onopen = () => {
                didOpen = true;
                setStatus("connected");
            };

            ws.onmessage = (ev) => {
                if (ev.data instanceof ArrayBuffer) {
                    // Binary frame = raw terminal output → write directly to xterm
                    if (terminalRef.current) {
                        terminalRef.current.write(new Uint8Array(ev.data));
                    }
                } else {
                    // Text frame = JSON control message (error, exit)
                    try {
                        const msg: ExecControlMsg = JSON.parse(ev.data);
                        switch (msg.type) {
                            case "error":
                                setStatus("error");
                                if (msg.message && terminalRef.current) {
                                    terminalRef.current.write(`\r\n\x1b[31mError: ${msg.message}\x1b[0m\r\n`);
                                }
                                break;
                            case "exit":
                                setStatus("exited");
                                setExitCode(msg.code ?? null);
                                if (terminalRef.current) {
                                    terminalRef.current.write(`\r\n\x1b[2m[Process exited with code ${msg.code ?? "?"}]\x1b[0m\r\n`);
                                }
                                break;
                        }
                    } catch {
                        // ignore malformed
                    }
                }
            };

            ws.onclose = () => {
                setStatus((prev) => (prev === "exited" || prev === "error" ? prev : "disconnected"));
            };

            ws.onerror = () => {
                setStatus("error");
                if (!didOpen && terminalRef.current) {
                    terminalRef.current.write("\r\n\x1b[31mFailed to connect. The container may not have a shell.\x1b[0m\r\n");
                }
            };
        },
        [agentUrl, token, containerId, nodeId, shell, user],
    );

    const disconnect = useCallback(() => {
        if (wsRef.current) {
            wsRef.current.close();
            wsRef.current = null;
        }
        setStatus("disconnected");
    }, []);

    return { status, exitCode, attachTerminal, sendInput, sendResize, connect, disconnect };
}
