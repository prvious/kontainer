import { useState } from "react";
import { useTaskDetail } from "@/hooks/use-task-detail";
import { LogViewer } from "@/components/log-viewer";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetDescription } from "@/components/ui/sheet";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { AlertCircle, Container, HardDrive, Key, Lock, Network, ScrollText } from "lucide-react";

function StateBadge({ state }: { state: string }) {
    const variants: Record<string, "default" | "secondary" | "destructive"> = {
        running: "default",
        complete: "secondary",
        failed: "destructive",
        rejected: "destructive",
        shutdown: "secondary",
        preparing: "secondary",
        ready: "secondary",
        starting: "secondary",
        assigned: "secondary",
        accepted: "secondary",
        pending: "secondary",
    };

    return <Badge variant={variants[state] ?? "secondary"}>{state}</Badge>;
}

function LoadingState() {
    return (
        <div className='space-y-4 p-4'>
            {[1, 2, 3, 4, 5].map((i) => (
                <Skeleton key={i} className='h-6 w-full' />
            ))}
        </div>
    );
}

function ErrorState() {
    return (
        <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
            <AlertCircle className='size-5' />
            <p>Failed to load task details</p>
        </div>
    );
}

export function TaskDetailSheet({ taskId, open, onOpenChange }: { taskId: string | null; open: boolean; onOpenChange: (open: boolean) => void }) {
    const { data, isLoading, error } = useTaskDetail(taskId);
    const [showLogs, setShowLogs] = useState(false);

    return (
        <Sheet open={open} onOpenChange={onOpenChange}>
            <SheetContent side='right' className='overflow-y-auto sm:max-w-lg'>
                <SheetHeader>
                    <SheetTitle>Task Detail</SheetTitle>
                    <SheetDescription>Container-level information for this task</SheetDescription>
                </SheetHeader>

                {isLoading ? (
                    <LoadingState />
                ) : error || !data ? (
                    <ErrorState />
                ) : (
                    <div className='space-y-6 px-4 pb-6'>
                        {/* State + Identity */}
                        <div className='space-y-3'>
                            <div className='flex items-center gap-2'>
                                <StateBadge state={data.state} />
                                <span className='text-xs text-muted-foreground'>desired:</span>
                                <StateBadge state={data.desiredState} />
                            </div>

                            {data.error && (
                                <div className='rounded-md border border-destructive/30 bg-destructive/5 p-3'>
                                    <p className='text-xs text-destructive'>{data.error}</p>
                                </div>
                            )}

                            <dl className='grid grid-cols-[100px_1fr] gap-x-3 gap-y-2 text-sm'>
                                <dt className='text-muted-foreground'>Task ID</dt>
                                <dd className='truncate font-mono text-xs'>{data.id}</dd>

                                <dt className='text-muted-foreground'>Slot</dt>
                                <dd>{data.slot || "-"}</dd>

                                <dt className='text-muted-foreground'>Node</dt>
                                <dd>{data.nodeName || "-"}</dd>

                                <dt className='text-muted-foreground'>Image</dt>
                                <dd className='truncate font-mono text-xs'>{data.image || "-"}</dd>
                            </dl>
                        </div>

                        <Separator />

                        {/* Container Info */}
                        <div className='space-y-2'>
                            <div className='flex items-center gap-1.5 text-sm font-medium'>
                                <Container className='size-3.5' /> Container
                            </div>
                            <dl className='grid grid-cols-[100px_1fr] gap-x-3 gap-y-2 text-sm'>
                                <dt className='text-muted-foreground'>Container ID</dt>
                                <dd className='truncate font-mono text-xs'>{data.containerId || "-"}</dd>

                                <dt className='text-muted-foreground'>PID</dt>
                                <dd className='font-mono text-xs'>{data.pid || "-"}</dd>

                                <dt className='text-muted-foreground'>Exit Code</dt>
                                <dd>
                                    {data.exitCode !== null ? (
                                        <span className={`font-mono text-xs ${data.exitCode !== 0 ? "text-destructive" : ""}`}>{data.exitCode}</span>
                                    ) : (
                                        <span className='text-xs text-muted-foreground'>-</span>
                                    )}
                                </dd>

                                <dt className='text-muted-foreground'>Created</dt>
                                <dd className='text-xs'>{new Date(data.createdAt).toLocaleString()}</dd>

                                <dt className='text-muted-foreground'>Updated</dt>
                                <dd className='text-xs'>{new Date(data.updatedAt).toLocaleString()}</dd>
                            </dl>
                        </div>

                        {/* Environment Variables */}
                        {data.env.length > 0 && (
                            <>
                                <Separator />
                                <div className='space-y-2'>
                                    <div className='flex items-center gap-1.5 text-sm font-medium'>
                                        <Key className='size-3.5' /> Environment ({data.env.length})
                                    </div>
                                    <Table>
                                        <TableHeader>
                                            <TableRow>
                                                <TableHead>Key</TableHead>
                                                <TableHead>Value</TableHead>
                                            </TableRow>
                                        </TableHeader>
                                        <TableBody>
                                            {data.env.map((env, i) => (
                                                <TableRow key={i}>
                                                    <TableCell className='font-mono text-xs font-medium'>{env.key}</TableCell>
                                                    <TableCell className='max-w-[200px] truncate font-mono text-xs text-muted-foreground'>
                                                        {env.value}
                                                    </TableCell>
                                                </TableRow>
                                            ))}
                                        </TableBody>
                                    </Table>
                                </div>
                            </>
                        )}

                        {/* Mounts */}
                        {data.mounts.length > 0 && (
                            <>
                                <Separator />
                                <div className='space-y-2'>
                                    <div className='flex items-center gap-1.5 text-sm font-medium'>
                                        <HardDrive className='size-3.5' /> Mounts ({data.mounts.length})
                                    </div>
                                    <Table>
                                        <TableHeader>
                                            <TableRow>
                                                <TableHead>Type</TableHead>
                                                <TableHead>Source</TableHead>
                                                <TableHead>Target</TableHead>
                                                <TableHead>Access</TableHead>
                                            </TableRow>
                                        </TableHeader>
                                        <TableBody>
                                            {data.mounts.map((m, i) => (
                                                <TableRow key={i}>
                                                    <TableCell>
                                                        <Badge variant='secondary'>{m.type}</Badge>
                                                    </TableCell>
                                                    <TableCell className='max-w-[120px] truncate font-mono text-xs'>{m.source}</TableCell>
                                                    <TableCell className='max-w-[120px] truncate font-mono text-xs'>{m.target}</TableCell>
                                                    <TableCell>
                                                        {m.readOnly ? (
                                                            <div className='flex items-center gap-1 text-xs text-muted-foreground'>
                                                                <Lock className='size-3' /> RO
                                                            </div>
                                                        ) : (
                                                            <span className='text-xs'>RW</span>
                                                        )}
                                                    </TableCell>
                                                </TableRow>
                                            ))}
                                        </TableBody>
                                    </Table>
                                </div>
                            </>
                        )}

                        {/* Networks */}
                        {data.networks.length > 0 && (
                            <>
                                <Separator />
                                <div className='space-y-2'>
                                    <div className='flex items-center gap-1.5 text-sm font-medium'>
                                        <Network className='size-3.5' /> Networks ({data.networks.length})
                                    </div>
                                    <Table>
                                        <TableHeader>
                                            <TableRow>
                                                <TableHead>Name</TableHead>
                                                <TableHead>Addresses</TableHead>
                                            </TableRow>
                                        </TableHeader>
                                        <TableBody>
                                            {data.networks.map((net, i) => (
                                                <TableRow key={i}>
                                                    <TableCell className='font-mono text-xs'>{net.name}</TableCell>
                                                    <TableCell className='font-mono text-xs text-muted-foreground'>{net.addresses.join(", ")}</TableCell>
                                                </TableRow>
                                            ))}
                                        </TableBody>
                                    </Table>
                                </div>
                            </>
                        )}

                        {/* Logs */}
                        <Separator />
                        <div className='space-y-2'>
                            <div className='flex items-center justify-between'>
                                <div className='flex items-center gap-1.5 text-sm font-medium'>
                                    <ScrollText className='size-3.5' /> Logs
                                </div>
                                <Button variant='outline' size='sm' onClick={() => setShowLogs(!showLogs)} data-testid='toggle-task-logs'>
                                    {showLogs ? "Hide" : "Show"}
                                </Button>
                            </div>
                            {showLogs && <LogViewer taskId={data.id} compact />}
                        </div>
                    </div>
                )}
            </SheetContent>
        </Sheet>
    );
}
