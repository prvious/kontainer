import { createFileRoute } from "@tanstack/react-router";
import { PageHeader } from "@/components/page-header";
import { Card, CardContent } from "@/components/ui/card";
import { Activity } from "lucide-react";

function TasksPage() {
    return (
        <>
            <PageHeader title='Tasks' description='Deployments and running operations' />
            <div className='p-6'>
                <Card>
                    <CardContent className='flex flex-col items-center justify-center gap-3 py-16'>
                        <div className='flex size-12 items-center justify-center rounded-lg bg-muted'>
                            <Activity className='size-6 text-muted-foreground' />
                        </div>
                        <div className='text-center'>
                            <p className='text-sm font-medium'>No tasks yet</p>
                            <p className='mt-1 text-xs text-muted-foreground'>Deployment tasks and operations will appear here</p>
                        </div>
                    </CardContent>
                </Card>
            </div>
        </>
    );
}

export const Route = createFileRoute("/_authenticated/tasks/")({
    component: TasksPage,
});
