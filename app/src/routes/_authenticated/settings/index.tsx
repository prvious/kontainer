import { createFileRoute } from "@tanstack/react-router";
import { Card, CardContent } from "@/components/ui/card";
import { Settings } from "lucide-react";

function SettingsPage() {
    return (
        <div className='p-6'>
            <div className='mb-6'>
                <h1 className='text-xl font-semibold tracking-tight'>Settings</h1>
                <p className='mt-1 text-sm text-muted-foreground'>Application and cluster configuration</p>
            </div>
            <Card>
                <CardContent className='flex flex-col items-center justify-center gap-3 py-16'>
                    <div className='flex size-12 items-center justify-center rounded-lg bg-primary/10'>
                        <Settings className='size-6 text-primary' />
                    </div>
                    <div className='text-center'>
                        <p className='text-sm font-medium'>Settings coming soon</p>
                        <p className='mt-1 text-xs text-muted-foreground'>Configure notifications, appearance, and cluster preferences.</p>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/settings/")({
    component: SettingsPage,
});
