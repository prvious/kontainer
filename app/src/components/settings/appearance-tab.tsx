import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useAppearance } from "@/hooks/use-appearance";
import { colorSchemeFrom } from "@/types/settings";

const SCHEME_OPTIONS = [
    { value: "system", label: "System" },
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" },
] as const;

export function AppearanceSection() {
    const { scheme, setScheme } = useAppearance();

    return (
        <section>
            <h2 className='mb-3 border-b border-border pb-2 font-mono text-xs tracking-widest text-muted-foreground uppercase'>Appearance</h2>
            <div className='flex items-center justify-between py-3'>
                <div className='space-y-0.5'>
                    <Label htmlFor='scheme-select'>Theme</Label>
                    <p className='text-xs text-muted-foreground'>Follow the system appearance or pick light or dark</p>
                </div>
                <Select value={scheme} onValueChange={(value) => value && setScheme(colorSchemeFrom(value))} items={SCHEME_OPTIONS}>
                    <SelectTrigger id='scheme-select' className='w-48'>
                        <SelectValue placeholder='Select theme' />
                    </SelectTrigger>
                    <SelectContent>
                        {SCHEME_OPTIONS.map((option) => (
                            <SelectItem key={option.value} value={option.value}>
                                {option.label}
                            </SelectItem>
                        ))}
                    </SelectContent>
                </Select>
            </div>
        </section>
    );
}
