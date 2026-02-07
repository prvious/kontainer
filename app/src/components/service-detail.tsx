import { useServiceDetail } from "@/hooks/use-service-detail";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { AlertCircle } from "lucide-react";

function formatTimestamp(iso: string): string {
    return new Date(iso).toLocaleString();
}

function LoadingSkeleton() {
    return (
        <div className='space-y-4'>
            <Card>
                <CardHeader>
                    <Skeleton className='h-5 w-40' />
                </CardHeader>
                <CardContent>
                    <div className='space-y-2'>
                        <Skeleton className='h-4 w-64' />
                        <Skeleton className='h-4 w-48' />
                        <Skeleton className='h-4 w-56' />
                    </div>
                </CardContent>
            </Card>
            {[1, 2, 3].map((i) => (
                <Card key={i}>
                    <CardHeader>
                        <Skeleton className='h-5 w-32' />
                    </CardHeader>
                    <CardContent>
                        <Skeleton className='h-4 w-full' />
                    </CardContent>
                </Card>
            ))}
        </div>
    );
}

export function ServiceDetail({ serviceId }: { serviceId: string }) {
    const { data, isLoading, error } = useServiceDetail(serviceId);

    if (isLoading) {
        return <LoadingSkeleton />;
    }

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                <AlertCircle className='size-5' />
                <p>Failed to load service details</p>
            </div>
        );
    }

    if (!data) return null;

    return (
        <div className='space-y-4'>
            {/* Overview */}
            <Card>
                <CardHeader>
                    <CardTitle>Overview</CardTitle>
                </CardHeader>
                <CardContent>
                    <dl className='grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm'>
                        <dt className='text-muted-foreground'>Name</dt>
                        <dd className='font-medium'>{data.name}</dd>

                        <dt className='text-muted-foreground'>Image</dt>
                        <dd className='font-mono text-xs'>{data.image}</dd>

                        <dt className='text-muted-foreground'>Mode</dt>
                        <dd>
                            <Badge variant='secondary'>{data.mode}</Badge>
                            {data.replicas !== null && (
                                <span className='ml-2 text-muted-foreground'>
                                    {data.replicas} {data.replicas === 1 ? "replica" : "replicas"}
                                </span>
                            )}
                        </dd>

                        {data.stack && (
                            <>
                                <dt className='text-muted-foreground'>Stack</dt>
                                <dd>{data.stack}</dd>
                            </>
                        )}

                        <dt className='text-muted-foreground'>Created</dt>
                        <dd>{formatTimestamp(data.createdAt)}</dd>

                        <dt className='text-muted-foreground'>Updated</dt>
                        <dd>{formatTimestamp(data.updatedAt)}</dd>
                    </dl>
                </CardContent>
            </Card>

            {/* Ports */}
            <Card>
                <CardHeader>
                    <CardTitle>Ports</CardTitle>
                </CardHeader>
                <CardContent>
                    {data.ports.length === 0 ? (
                        <p className='text-sm text-muted-foreground'>None</p>
                    ) : (
                        <table className='w-full text-sm'>
                            <thead>
                                <tr className='border-b text-left text-muted-foreground'>
                                    <th className='pb-2 font-medium'>Published</th>
                                    <th className='pb-2 font-medium'>Target</th>
                                    <th className='pb-2 font-medium'>Protocol</th>
                                    <th className='pb-2 font-medium'>Mode</th>
                                </tr>
                            </thead>
                            <tbody>
                                {data.ports.map((port, i) => (
                                    <tr key={i} className='border-b last:border-0'>
                                        <td className='py-2'>{port.publishedPort}</td>
                                        <td className='py-2'>{port.targetPort}</td>
                                        <td className='py-2'>{port.protocol}</td>
                                        <td className='py-2'>{port.publishMode}</td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    )}
                </CardContent>
            </Card>

            {/* Environment */}
            <Card>
                <CardHeader>
                    <CardTitle>Environment</CardTitle>
                </CardHeader>
                <CardContent>
                    {data.env.length === 0 ? (
                        <p className='text-sm text-muted-foreground'>None</p>
                    ) : (
                        <table className='w-full text-sm'>
                            <thead>
                                <tr className='border-b text-left text-muted-foreground'>
                                    <th className='pb-2 font-medium'>Key</th>
                                    <th className='pb-2 font-medium'>Value</th>
                                </tr>
                            </thead>
                            <tbody>
                                {data.env.map((env, i) => (
                                    <tr key={i} className='border-b last:border-0'>
                                        <td className='py-2 font-mono text-xs'>{env.key}</td>
                                        <td className='py-2 font-mono text-xs'>{env.value}</td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    )}
                </CardContent>
            </Card>

            {/* Mounts */}
            <Card>
                <CardHeader>
                    <CardTitle>Mounts</CardTitle>
                </CardHeader>
                <CardContent>
                    {data.mounts.length === 0 ? (
                        <p className='text-sm text-muted-foreground'>None</p>
                    ) : (
                        <table className='w-full text-sm'>
                            <thead>
                                <tr className='border-b text-left text-muted-foreground'>
                                    <th className='pb-2 font-medium'>Type</th>
                                    <th className='pb-2 font-medium'>Source</th>
                                    <th className='pb-2 font-medium'>Target</th>
                                    <th className='pb-2 font-medium'>Read Only</th>
                                </tr>
                            </thead>
                            <tbody>
                                {data.mounts.map((m, i) => (
                                    <tr key={i} className='border-b last:border-0'>
                                        <td className='py-2'>{m.type}</td>
                                        <td className='py-2 font-mono text-xs'>{m.source}</td>
                                        <td className='py-2 font-mono text-xs'>{m.target}</td>
                                        <td className='py-2'>{m.readOnly ? <Badge variant='secondary'>read-only</Badge> : "no"}</td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    )}
                </CardContent>
            </Card>

            {/* Constraints */}
            <Card>
                <CardHeader>
                    <CardTitle>Constraints</CardTitle>
                </CardHeader>
                <CardContent>
                    {data.constraints.length === 0 ? (
                        <p className='text-sm text-muted-foreground'>None</p>
                    ) : (
                        <ul className='space-y-1'>
                            {data.constraints.map((c, i) => (
                                <li key={i} className='font-mono text-sm'>
                                    {c}
                                </li>
                            ))}
                        </ul>
                    )}
                </CardContent>
            </Card>

            {/* Update Status */}
            {data.updateStatus && (
                <Card>
                    <CardHeader>
                        <CardTitle>Update Status</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <dl className='grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm'>
                            <dt className='text-muted-foreground'>State</dt>
                            <dd>
                                <Badge variant={data.updateStatus.state === "completed" ? "default" : "secondary"}>{data.updateStatus.state}</Badge>
                            </dd>

                            {data.updateStatus.startedAt && (
                                <>
                                    <dt className='text-muted-foreground'>Started</dt>
                                    <dd>{formatTimestamp(data.updateStatus.startedAt)}</dd>
                                </>
                            )}

                            {data.updateStatus.completedAt && (
                                <>
                                    <dt className='text-muted-foreground'>Completed</dt>
                                    <dd>{formatTimestamp(data.updateStatus.completedAt)}</dd>
                                </>
                            )}

                            {data.updateStatus.message && (
                                <>
                                    <dt className='text-muted-foreground'>Message</dt>
                                    <dd>{data.updateStatus.message}</dd>
                                </>
                            )}
                        </dl>
                    </CardContent>
                </Card>
            )}
        </div>
    );
}
