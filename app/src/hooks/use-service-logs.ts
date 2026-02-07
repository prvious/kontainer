import { useLogStream } from "@/hooks/use-log-stream";
import type { LogOptions } from "@/hooks/use-log-stream";

export type { LogEntry, LogOptions, ConnectionStatus } from "@/hooks/use-log-stream";

export function useServiceLogs(serviceId: string, options: LogOptions) {
    return useLogStream(`/api/services/${serviceId}/logs`, options);
}
