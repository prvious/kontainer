import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useAppearance } from "@/hooks/use-appearance";

export function AppearanceTab() {
    const { font, setFont, fonts } = useAppearance();

    return (
        <Card>
            <CardHeader>
                <CardTitle>Appearance</CardTitle>
                <CardDescription>Customize how the app looks</CardDescription>
            </CardHeader>
            <CardContent>
                <div className='flex items-center justify-between'>
                    <div className='space-y-0.5'>
                        <Label htmlFor='font-select'>Font</Label>
                        <p className='text-xs text-muted-foreground'>The typeface used across the app UI</p>
                    </div>
                    <Select value={font} onValueChange={(v) => v && setFont(v)}>
                        <SelectTrigger id='font-select' className='w-48'>
                            <SelectValue placeholder='Select font' />
                        </SelectTrigger>
                        <SelectContent>
                            {fonts.map((f) => (
                                <SelectItem key={f.id} value={f.id}>
                                    {f.label}
                                </SelectItem>
                            ))}
                        </SelectContent>
                    </Select>
                </div>
            </CardContent>
        </Card>
    );
}
