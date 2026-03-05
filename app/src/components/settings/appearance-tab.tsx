import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useAppearance } from "@/hooks/use-appearance";

export function AppearanceSection() {
    const { font, setFont, fonts } = useAppearance();

    return (
        <section>
            <h2 className='mb-3 border-b border-border pb-2 font-mono text-xs tracking-widest text-muted-foreground uppercase'>Appearance</h2>
            <div className='flex items-center justify-between py-3'>
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
        </section>
    );
}
