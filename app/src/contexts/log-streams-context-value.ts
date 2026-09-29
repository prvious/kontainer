import { createContext } from "react";
import type { LogBuffer } from "@/components/logs/log-buffer";

/** One open Logs tab: a service, its buffer, and the stream currently feeding it. */
export interface LogTab {
    id: string;
    serviceId: string;
    /** Set once `logs_open` answers; null while opening or after it failed. */
    streamId: string | null;
    buffer: LogBuffer;
    /** The window the current stream opened with, for the empty state. */
    window: { since: number | null; tail: number; openedAt: number };
}

export interface LogStreamsContextValue {
    tabs: LogTab[];
    activeId: string | null;
    select(id: string): void;
    /** Focus the service's tab, or open one. */
    open(serviceId: string): void;
    close(id: string): void;
    /** After an end: reopen from the last line, keeping the lines already there. */
    resume(id: string): void;
    /** G19: start over with the last hour, up to 5,000 lines. */
    loadPreviousHour(id: string): void;
    retryNow(id: string): void;
}

export const LogStreamsContext = createContext<LogStreamsContextValue | null>(null);
