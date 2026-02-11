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

export function GeneralTab() {
    const { refreshInterval, setRefreshInterval } = useSettings();

    const currentValue = refreshInterval === false ? "false" : String(refreshInterval);

    function handleRefreshChange(value: string | null) {
        if (!value) return;
        if (value === "false") {
            setRefreshInterval(false);
        } else {
            setRefreshInterval(Number(value));
        }
    }

    return (
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
    );
}
