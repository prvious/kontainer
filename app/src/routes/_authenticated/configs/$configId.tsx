import { createFileRoute, Link } from "@tanstack/react-router";
import { useConfigDetail } from "@/hooks/use-config-detail";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { AlertCircle, ArrowLeft } from "lucide-react";
import { CodeEditor } from "@/components/code-editor";

function ConfigDetailPage() {
    const { configId } = Route.useParams();
    const { data: config, isLoading, error } = useConfigDetail(configId);

    if (isLoading) {
        return (
            <div className='p-6'>
                <Skeleton className='mb-6 h-8 w-48' />
                <div className='grid gap-6 lg:grid-cols-3'>
                    <div className='lg:col-span-2'>
                        <Skeleton className='h-[400px] w-full rounded-lg' />
                    </div>
                    <div>
                        <Skeleton className='h-48 w-full rounded-lg' />
                    </div>
                </div>
            </div>
        );
    }

    if (error || !config) {
        return (
            <div className='p-6'>
                <Link to='/configs'>
                    <Button variant='ghost' size='sm' className='mb-4'>
                        <ArrowLeft className='size-4' />
                        Back to Configs
                    </Button>
                </Link>
                <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                    <AlertCircle className='size-5' />
                    <p>Failed to load config</p>
                </div>
            </div>
        );
    }

    return (
        <div className='flex h-full flex-col p-6'>
            <div className='mb-6 flex items-center gap-3'>
                <Link to='/configs'>
                    <Button variant='ghost' size='icon-sm'>
                        <ArrowLeft className='size-4' />
                    </Button>
                </Link>
                <h1 className='text-xl font-semibold tracking-tight'>{config.name}</h1>
            </div>

            <div className='grid min-h-0 flex-1 gap-6 lg:grid-cols-3'>
                <div className='flex lg:col-span-2'>
                    <Card className='flex flex-1 flex-col'>
                        <CardHeader>
                            <CardTitle className='text-sm font-medium'>Config Data</CardTitle>
                        </CardHeader>
                        <CardContent className='flex min-h-0 flex-1 flex-col'>
                            <CodeEditor value={config.data} readOnly className='min-h-0 flex-1' minHeight='300px' fill />
                        </CardContent>
                    </Card>
                </div>

                <div>
                    <Card>
                        <CardHeader>
                            <CardTitle className='text-sm font-medium'>Metadata</CardTitle>
                        </CardHeader>
                        <CardContent className='grid gap-3 text-sm'>
                            <div>
                                <span className='text-muted-foreground'>ID</span>
                                <p className='font-mono text-xs'>{config.id}</p>
                            </div>
                            <div>
                                <span className='text-muted-foreground'>Labels</span>
                                {Object.keys(config.labels).length === 0 ? (
                                    <p className='text-xs text-muted-foreground'>None</p>
                                ) : (
                                    <div className='mt-1 flex flex-wrap gap-1'>
                                        {Object.entries(config.labels).map(([k, v]) => (
                                            <Badge key={k} variant='secondary' className='text-[10px]'>
                                                {k}={v}
                                            </Badge>
                                        ))}
                                    </div>
                                )}
                            </div>
                            <div>
                                <span className='text-muted-foreground'>Created</span>
                                <p className='text-xs'>{new Date(config.createdAt).toLocaleString()}</p>
                            </div>
                            <div>
                                <span className='text-muted-foreground'>Updated</span>
                                <p className='text-xs'>{new Date(config.updatedAt).toLocaleString()}</p>
                            </div>
                        </CardContent>
                    </Card>
                </div>
            </div>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/configs/$configId")({
    component: ConfigDetailPage,
});
