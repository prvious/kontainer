import { useState } from "react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useServiceDetail } from "@/hooks/use-service-detail";
import { useScaleService } from "@/hooks/use-scale-service";
import { useRestartService } from "@/hooks/use-restart-service";
import { useRollbackService } from "@/hooks/use-rollback-service";
import { EnvEditor } from "@/components/env-editor";
import { ImageEditor } from "@/components/image-editor";
import { PageHeader } from "@/components/page-header";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Separator } from "@/components/ui/separator";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { LogViewer } from "@/components/log-viewer";
import { ServiceTasks } from "@/components/service-tasks";
import {
    AlertCircle,
    ArrowLeft,
    Container,
    Globe,
    HardDrive,
    Key,
    Layers,
    ListTodo,
    Loader2,
    Lock,
    Minus,
    Network,
    Plus,
    RefreshCw,
    RotateCcw,
    ScrollText,
    Settings,
} from "lucide-react";

function formatTimestamp(iso: string): string {
    return new Date(iso).toLocaleString();
}

function formatRelativeTime(iso: string): string {
    const seconds = Math.floor((Date.now() - new Date(iso).getTime()) / 1000);
    if (seconds < 60) return "just now";
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
}

function LoadingSkeleton() {
    return (
        <div className='space-y-6 p-6'>
            <div className='grid gap-4 sm:grid-cols-3'>
                {[1, 2, 3].map((i) => (
                    <Card key={i}>
                        <CardContent className='p-4'>
                            <Skeleton className='mb-2 h-3 w-16' />
                            <Skeleton className='h-5 w-32' />
                        </CardContent>
                    </Card>
                ))}
            </div>
            <Skeleton className='h-10 w-64' />
            <Card>
                <CardContent className='space-y-3 p-6'>
                    {[1, 2, 3, 4].map((i) => (
                        <Skeleton key={i} className='h-4 w-full' />
                    ))}
                </CardContent>
            </Card>
        </div>
    );
}

function ScaleControls({ serviceId, replicas }: { serviceId: string; replicas: number }) {
    const [localReplicas, setLocalReplicas] = useState(replicas);
    const { mutate, isPending } = useScaleService(serviceId);

    function handleScale(newReplicas: number) {
        const clamped = Math.max(0, newReplicas);
        setLocalReplicas(clamped);
        mutate(clamped);
    }

    return (
        <div className='mt-1.5 flex items-center gap-1.5'>
            <Button
                variant='outline'
                size='sm'
                className='size-7 p-0'
                disabled={isPending || localReplicas <= 0}
                onClick={() => handleScale(localReplicas - 1)}
                aria-label='Decrease replicas'
            >
                {isPending ? <Loader2 className='size-3 animate-spin' /> : <Minus className='size-3' />}
            </Button>
            <Input
                type='number'
                min={0}
                value={localReplicas}
                onChange={(e) => {
                    const val = parseInt(e.target.value, 10);
                    if (!isNaN(val) && val >= 0) {
                        setLocalReplicas(val);
                    }
                }}
                onBlur={() => {
                    if (localReplicas !== replicas) {
                        mutate(localReplicas);
                    }
                }}
                onKeyDown={(e) => {
                    if (e.key === "Enter" && localReplicas !== replicas) {
                        mutate(localReplicas);
                    }
                }}
                className='h-7 w-14 [appearance:textfield] text-center font-mono text-sm [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none'
                disabled={isPending}
                aria-label='Replica count'
            />
            <Button
                variant='outline'
                size='sm'
                className='size-7 p-0'
                disabled={isPending}
                onClick={() => handleScale(localReplicas + 1)}
                aria-label='Increase replicas'
            >
                {isPending ? <Loader2 className='size-3 animate-spin' /> : <Plus className='size-3' />}
            </Button>
        </div>
    );
}

function ServiceDetailPage() {
    const { serviceId } = Route.useParams();
    const { data, isLoading, error } = useServiceDetail(serviceId);
    const { mutate: restart, isPending: isRestarting } = useRestartService(serviceId);
    const { mutate: rollback, isPending: isRollingBack } = useRollbackService(serviceId);

    if (isLoading) {
        return (
            <>
                <PageHeader title='Service' />
                <LoadingSkeleton />
            </>
        );
    }

    if (error || !data) {
        return (
            <>
                <PageHeader
                    title='Service'
                    actions={
                        <Button variant='ghost' size='sm' render={(props) => <Link {...props} to='/services' />}>
                            <ArrowLeft className='size-4' /> Back
                        </Button>
                    }
                />
                <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                    <AlertCircle className='size-5' />
                    <p>Failed to load service details</p>
                </div>
            </>
        );
    }

    const isGlobal = data.mode === "global";
    const healthy = isGlobal || (data.replicas !== null && data.replicas > 0);

    return (
        <>
            <PageHeader
                title={data.name}
                description={data.image}
                actions={
                    <>
                        {data.previousImage && (
                            <Button variant='outline' size='sm' disabled={isRollingBack} onClick={() => rollback()} aria-label='Rollback service'>
                                <RotateCcw className={`size-4 ${isRollingBack ? "animate-spin" : ""}`} />
                                Rollback
                            </Button>
                        )}
                        <Button variant='outline' size='sm' disabled={isRestarting} onClick={() => restart()} aria-label='Restart service'>
                            <RefreshCw className={`size-4 ${isRestarting ? "animate-spin" : ""}`} />
                            Restart
                        </Button>
                        <Button variant='ghost' size='sm' render={(props) => <Link {...props} to='/services' />}>
                            <ArrowLeft className='size-4' /> Back
                        </Button>
                    </>
                }
            />

            <div className='space-y-6 p-6'>
                {/* Quick stats row */}
                <div className='grid gap-4 sm:grid-cols-4'>
                    <Card className='animate-fade-in-up'>
                        <CardContent className='p-4'>
                            <p className='mb-1 text-xs text-muted-foreground'>Status</p>
                            <div className='flex items-center gap-2'>
                                <span className={`size-2 rounded-full ${healthy ? 'animate-pulse-dot bg-success' : 'animate-pulse-dot bg-destructive'}`} />
                                <span className='text-sm font-medium'>{healthy ? "Healthy" : "Degraded"}</span>
                            </div>
                        </CardContent>
                    </Card>

                    <Card className='animate-fade-in-up' style={{ animationDelay: "50ms" }}>
                        <CardContent className='p-4'>
                            <p className='mb-1 text-xs text-muted-foreground'>Mode</p>
                            <Badge variant='secondary'>{data.mode}</Badge>
                            {isGlobal ? null : data.replicas !== null ? <ScaleControls serviceId={serviceId} replicas={data.replicas} /> : null}
                        </CardContent>
                    </Card>

                    <Card className='animate-fade-in-up' style={{ animationDelay: "100ms" }}>
                        <CardContent className='p-4'>
                            <p className='mb-1 text-xs text-muted-foreground'>Stack</p>
                            <div className='flex items-center gap-1.5'>
                                <Layers className='size-3.5 text-muted-foreground' />
                                <span className='text-sm font-medium'>{data.stack || "Standalone"}</span>
                            </div>
                        </CardContent>
                    </Card>

                    <Card className='animate-fade-in-up' style={{ animationDelay: "150ms" }}>
                        <CardContent className='p-4'>
                            <p className='mb-1 text-xs text-muted-foreground'>Last Updated</p>
                            <span className='text-sm font-medium'>{formatRelativeTime(data.updatedAt)}</span>
                        </CardContent>
                    </Card>
                </div>

                {/* Update Status banner */}
                {data.updateStatus && (
                    <Card className='animate-fade-in-up border-warning/30 bg-warning/5' style={{ animationDelay: "200ms" }}>
                        <CardContent className='flex items-center gap-4 p-4'>
                            <Settings className='size-5 text-warning' />
                            <div className='flex-1'>
                                <p className='text-sm font-medium'>
                                    Update {data.updateStatus.state}
                                    {data.updateStatus.message && ` — ${data.updateStatus.message}`}
                                </p>
                                <div className='mt-1 flex gap-4 text-xs text-muted-foreground'>
                                    {data.updateStatus.startedAt && <span>Started {formatTimestamp(data.updateStatus.startedAt)}</span>}
                                    {data.updateStatus.completedAt && <span>Completed {formatTimestamp(data.updateStatus.completedAt)}</span>}
                                </div>
                            </div>
                            <Badge variant={data.updateStatus.state === "completed" ? "default" : "secondary"}>{data.updateStatus.state}</Badge>
                        </CardContent>
                    </Card>
                )}

                {/* Tabbed detail sections */}
                <Tabs defaultValue='overview' className='animate-fade-in-up' style={{ animationDelay: "250ms" }}>
                    <TabsList>
                        <TabsTrigger value='overview'>
                            <Container className='size-3.5' /> Overview
                        </TabsTrigger>
                        <TabsTrigger value='network'>
                            <Network className='size-3.5' /> Network
                        </TabsTrigger>
                        <TabsTrigger value='env'>
                            <Key className='size-3.5' /> Environment
                        </TabsTrigger>
                        <TabsTrigger value='storage'>
                            <HardDrive className='size-3.5' /> Storage
                        </TabsTrigger>
                        <TabsTrigger value='tasks'>
                            <ListTodo className='size-3.5' /> Tasks
                        </TabsTrigger>
                        <TabsTrigger value='logs'>
                            <ScrollText className='size-3.5' /> Logs
                        </TabsTrigger>
                    </TabsList>

                    <TabsContent value='overview' className='mt-4'>
                        <Card>
                            <CardHeader>
                                <CardTitle className='text-sm'>Configuration</CardTitle>
                            </CardHeader>
                            <CardContent>
                                <dl className='grid grid-cols-[140px_1fr] gap-x-4 gap-y-3 text-sm'>
                                    <dt className='text-muted-foreground'>Service ID</dt>
                                    <dd className='truncate font-mono text-xs'>{data.id}</dd>

                                    <dt className='text-muted-foreground'>Image</dt>
                                    <dd>
                                        <ImageEditor key={data.image} serviceId={serviceId} currentImage={data.image} />
                                    </dd>

                                    {data.previousImage && (
                                        <>
                                            <dt className='text-muted-foreground'>Previous Image</dt>
                                            <dd className='truncate font-mono text-xs text-muted-foreground'>{data.previousImage}</dd>
                                        </>
                                    )}

                                    <dt className='text-muted-foreground'>Created</dt>
                                    <dd>{formatTimestamp(data.createdAt)}</dd>

                                    <dt className='text-muted-foreground'>Updated</dt>
                                    <dd>{formatTimestamp(data.updatedAt)}</dd>
                                </dl>

                                {data.constraints.length > 0 && (
                                    <>
                                        <Separator className='my-4' />
                                        <div>
                                            <p className='mb-2 text-sm font-medium'>Constraints</p>
                                            <div className='flex flex-wrap gap-2'>
                                                {data.constraints.map((c, i) => (
                                                    <Badge key={i} variant='secondary' className='font-mono text-xs'>
                                                        {c}
                                                    </Badge>
                                                ))}
                                            </div>
                                        </div>
                                    </>
                                )}
                            </CardContent>
                        </Card>
                    </TabsContent>

                    <TabsContent value='network' className='mt-4'>
                        <Card>
                            <CardHeader>
                                <CardTitle className='text-sm'>Published Ports</CardTitle>
                            </CardHeader>
                            <CardContent>
                                {data.ports.length === 0 ? (
                                    <p className='text-sm text-muted-foreground'>No ports published</p>
                                ) : (
                                    <Table>
                                        <TableHeader>
                                            <TableRow>
                                                <TableHead>Published</TableHead>
                                                <TableHead>Target</TableHead>
                                                <TableHead>Protocol</TableHead>
                                                <TableHead>Mode</TableHead>
                                            </TableRow>
                                        </TableHeader>
                                        <TableBody>
                                            {data.ports.map((port, i) => (
                                                <TableRow key={i}>
                                                    <TableCell className='font-mono'>{port.publishedPort}</TableCell>
                                                    <TableCell className='font-mono'>{port.targetPort}</TableCell>
                                                    <TableCell>
                                                        <Badge variant='secondary'>{port.protocol}</Badge>
                                                    </TableCell>
                                                    <TableCell>
                                                        <div className='flex items-center gap-1.5'>
                                                            {port.publishMode === "ingress" ? (
                                                                <Globe className='size-3 text-muted-foreground' />
                                                            ) : (
                                                                <Container className='size-3 text-muted-foreground' />
                                                            )}
                                                            {port.publishMode}
                                                        </div>
                                                    </TableCell>
                                                </TableRow>
                                            ))}
                                        </TableBody>
                                    </Table>
                                )}
                            </CardContent>
                        </Card>
                    </TabsContent>

                    <TabsContent value='env' className='mt-4'>
                        <Card>
                            <CardHeader>
                                <CardTitle className='text-sm'>Environment Variables</CardTitle>
                            </CardHeader>
                            <CardContent>
                                <EnvEditor key={data.env.map((e) => `${e.key}=${e.value}`).join(",")} serviceId={serviceId} initialEnv={data.env} />
                            </CardContent>
                        </Card>
                    </TabsContent>

                    <TabsContent value='storage' className='mt-4'>
                        <Card>
                            <CardHeader>
                                <CardTitle className='text-sm'>Mounts</CardTitle>
                            </CardHeader>
                            <CardContent>
                                {data.mounts.length === 0 ? (
                                    <p className='text-sm text-muted-foreground'>No mounts configured</p>
                                ) : (
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
                                                    <TableCell className='max-w-xs truncate font-mono text-xs'>{m.source}</TableCell>
                                                    <TableCell className='max-w-xs truncate font-mono text-xs'>{m.target}</TableCell>
                                                    <TableCell>
                                                        {m.readOnly ? (
                                                            <div className='flex items-center gap-1.5 text-sm text-muted-foreground'>
                                                                <Lock className='size-3' /> Read-only
                                                            </div>
                                                        ) : (
                                                            <span className='text-sm'>Read-write</span>
                                                        )}
                                                    </TableCell>
                                                </TableRow>
                                            ))}
                                        </TableBody>
                                    </Table>
                                )}
                            </CardContent>
                        </Card>
                    </TabsContent>

                    <TabsContent value='tasks' className='mt-4'>
                        <ServiceTasks serviceId={serviceId} />
                    </TabsContent>

                    <TabsContent value='logs' className='mt-4'>
                        <LogViewer serviceId={serviceId} />
                    </TabsContent>
                </Tabs>
            </div>
        </>
    );
}

export const Route = createFileRoute("/_authenticated/services/$serviceId")({
    component: ServiceDetailPage,
});
