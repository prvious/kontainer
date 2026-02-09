import { createFileRoute } from "@tanstack/react-router";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useSettings } from "@/hooks/use-settings";

const REFRESH_OPTIONS = [
    { value: "3000", label: "3 seconds" },
    { value: "5000", label: "5 seconds" },
    { value: "10000", label: "10 seconds" },
    { value: "30000", label: "30 seconds" },
    { value: "false", label: "Off" },
] as const;

function SettingsPage() {
    const { refreshInterval, setRefreshInterval } = useSettings();

    const currentValue = refreshInterval === false ? "false" : String(refreshInterval);

    function handleRefreshChange(value: string) {
        if (value === "false") {
            setRefreshInterval(false);
        } else {
            setRefreshInterval(Number(value));
        }
    }

    return (
        <div className='p-6'>
            <div className='mb-6'>
                <h1 className='text-xl font-semibold tracking-tight'>Settings</h1>
                <p className='mt-1 text-sm text-muted-foreground'>Application and cluster configuration</p>
            </div>

            <div className='space-y-6'>
                <Card>
                    <CardHeader>
                        <CardTitle>App Settings</CardTitle>
                        <CardDescription>Configure how the desktop app behaves</CardDescription>
                    </CardHeader>
                    <CardContent>
                        <div className='flex items-center justify-between'>
                            <div className='space-y-0.5'>
                                <Label htmlFor='refresh-interval'>Refresh interval</Label>
                                <p className='text-xs text-muted-foreground'>How often data is polled from the agent</p>
                            </div>
                            <Select value={currentValue} onValueChange={handleRefreshChange} items={REFRESH_OPTIONS}>
                                <SelectTrigger id='refresh-interval' className='w-36'>
                                    <SelectValue placeholder='Select interval' />
                                </SelectTrigger>
                                <SelectContent>
                                    {REFRESH_OPTIONS.map((opt) => (
                                        <SelectItem key={opt.value} value={opt.value}>
                                            {opt.label}
                                        </SelectItem>
                                    ))}
                                </SelectContent>
                            </Select>
                        </div>
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader>
                        <CardTitle>Agent Settings</CardTitle>
                        <CardDescription>Settings stored on the connected agent</CardDescription>
                    </CardHeader>
                    <CardContent>
                        <p className='text-sm text-muted-foreground'>No agent settings available yet.</p>
                    </CardContent>
                </Card>
            </div>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/settings/")({
    component: SettingsPage,
});
