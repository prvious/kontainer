import { createFileRoute } from "@tanstack/react-router";
import { Card, CardContent } from "@/components/ui/card";
import { Activity } from "lucide-react";

function TasksPage() {
    return (
        <div className='p-6'>
            <div className='mb-6'>
                <h1 className='text-xl font-semibold tracking-tight'>Containers</h1>
                <p className='mt-1 text-sm text-muted-foreground'>Deployments and running operations</p>
            </div>
            <Card>
                <CardContent className='flex flex-col items-center justify-center gap-3 py-16'>
                    <div className='flex size-12 items-center justify-center rounded-lg bg-primary/10'>
                        <Activity className='size-6 text-primary' />
                    </div>
                    <div className='text-center'>
                        <p className='text-sm font-medium'>No tasks yet</p>
                        <p className='mt-1 text-xs text-muted-foreground'>Deployment tasks and operations will appear here</p>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/tasks/")({
    component: TasksPage,
});
