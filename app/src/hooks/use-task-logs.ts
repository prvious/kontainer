import { useLogStream } from "@/hooks/use-log-stream";
import type { LogOptions } from "@/hooks/use-log-stream";

export function useTaskLogs(taskId: string, options: LogOptions) {
    return useLogStream(`/api/tasks/${taskId}/logs`, options);
}
