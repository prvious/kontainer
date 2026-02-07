import { useCallback, useEffect, useRef, useState } from "react";
import { useLogStream, type LogOptions } from "@/hooks/use-log-stream";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Separator } from "@/components/ui/separator";
import { ArrowDown, Pause, Play, RotateCcw, Trash2, Wifi, WifiOff } from "lucide-react";

const TAIL_OPTIONS = [
    { value: "100", label: "100 lines" },
    { value: "500", label: "500 lines" },
    { value: "1000", label: "1000 lines" },
    { value: "all", label: "All" },
];

export function LogViewer({ serviceId, taskId, compact }: { serviceId?: string; taskId?: string; compact?: boolean }) {
    const path = taskId ? `/api/tasks/${taskId}/logs` : `/api/services/${serviceId}/logs`;

    const [options, setOptions] = useState<LogOptions>({
        tail: "100",
        timestamps: true,
        stdout: true,
        stderr: true,
    });

    const { logs, status, isPaused, clearLogs, pause, resume, reconnect } = useLogStream(path, options);

    const scrollRef = useRef<HTMLDivElement>(null);
    const [autoScroll, setAutoScroll] = useState(true);

    // Auto-scroll to bottom when new logs arrive
    useEffect(() => {
        if (autoScroll && scrollRef.current) {
            scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
        }
    }, [logs, autoScroll]);

    const handleScroll = useCallback(() => {
        if (!scrollRef.current) return;
        const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
        const atBottom = scrollHeight - scrollTop - clientHeight < 40;
        setAutoScroll(atBottom);
    }, []);

    const scrollToBottom = useCallback(() => {
        if (scrollRef.current) {
            scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
            setAutoScroll(true);
        }
    }, []);

    const statusColor = {
        connected: "bg-success",
        connecting: "bg-warning",
        disconnected: "bg-muted-foreground",
        error: "bg-destructive",
    }[status];

    return (
        <Card>
            <CardContent className='space-y-3 p-4'>
                {/* Toolbar */}
                <div className='flex flex-wrap items-center gap-3'>
                    {/* Connection status */}
                    <div className='flex items-center gap-1.5'>
                        <span className={`size-2 rounded-full ${statusColor}`} />
                        <span className='text-xs text-muted-foreground capitalize'>{status}</span>
                    </div>

                    <Separator orientation='vertical' className='h-5' />

                    {/* Tail select */}
                    <Select
                        value={options.tail}
                        onValueChange={(val) => {
                            if (val) setOptions((prev) => ({ ...prev, tail: val as string }));
                        }}
                    >
                        <SelectTrigger size='sm'>
                            <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                            {TAIL_OPTIONS.map((opt) => (
                                <SelectItem key={opt.value} value={opt.value}>
                                    {opt.label}
                                </SelectItem>
                            ))}
                        </SelectContent>
                    </Select>

                    <Separator orientation='vertical' className='h-5' />

                    {/* Stream toggles */}
                    <label className='flex items-center gap-1.5 text-xs'>
                        <Switch size='sm' checked={options.stdout} onCheckedChange={(val) => setOptions((prev) => ({ ...prev, stdout: val }))} />
                        stdout
                    </label>
                    <label className='flex items-center gap-1.5 text-xs'>
                        <Switch size='sm' checked={options.stderr} onCheckedChange={(val) => setOptions((prev) => ({ ...prev, stderr: val }))} />
                        stderr
                    </label>
                    <label className='flex items-center gap-1.5 text-xs'>
                        <Switch size='sm' checked={options.timestamps} onCheckedChange={(val) => setOptions((prev) => ({ ...prev, timestamps: val }))} />
                        timestamps
                    </label>

                    <Separator orientation='vertical' className='h-5' />

                    {/* Action buttons */}
                    <Button variant='ghost' size='icon-sm' onClick={isPaused ? resume : pause} title={isPaused ? "Resume" : "Pause"}>
                        {isPaused ? <Play className='size-3.5' /> : <Pause className='size-3.5' />}
                    </Button>
                    <Button variant='ghost' size='icon-sm' onClick={clearLogs} title='Clear logs'>
                        <Trash2 className='size-3.5' />
                    </Button>
                    <Button variant='ghost' size='icon-sm' onClick={reconnect} title='Reconnect'>
                        <RotateCcw className='size-3.5' />
                    </Button>
                    {!autoScroll && (
                        <Button variant='ghost' size='icon-sm' onClick={scrollToBottom} title='Scroll to bottom'>
                            <ArrowDown className='size-3.5' />
                        </Button>
                    )}

                    {/* Right-aligned info */}
                    <div className='ml-auto flex items-center gap-2'>
                        {isPaused && <Badge variant='secondary'>Paused</Badge>}
                        <span className='text-xs text-muted-foreground'>{logs.length} lines</span>
                        {status === "connected" ? <Wifi className='size-3.5 text-success' /> : <WifiOff className='size-3.5 text-muted-foreground' />}
                    </div>
                </div>

                {/* Log output */}
                <div
                    ref={scrollRef}
                    onScroll={handleScroll}
                    className={`${compact ? "h-[300px]" : "h-[500px]"} overflow-auto rounded-md border bg-background p-3 font-mono text-xs`}
                >
                    {logs.length === 0 ? (
                        <p className='text-muted-foreground'>
                            {status === "connecting" ? "Connecting..." : status === "connected" ? "Waiting for logs..." : "No logs"}
                        </p>
                    ) : (
                        logs.map((entry) => (
                            <div key={entry.id} className='leading-5'>
                                {options.timestamps && entry.timestamp && <span className='mr-2 text-muted-foreground'>{entry.timestamp}</span>}
                                <span className={entry.stream === "stderr" ? "text-destructive" : ""}>{entry.line}</span>
                            </div>
                        ))
                    )}
                </div>
            </CardContent>
        </Card>
    );
}
