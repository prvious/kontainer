import { Minus, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ButtonGroup } from "@/components/ui/button-group";
import { Input } from "@/components/ui/input";
import { fieldHelperId } from "@/components/kit/field-ids";
import { FONT_SIZE_MAX, FONT_SIZE_MIN } from "@/components/settings/settings-form";

/** The font size stepper: − and + move by one within 10–24; the field takes a typed size. */
export function FontSizeStepper({
    id,
    value,
    invalid,
    onChange,
    onBlur,
}: {
    id: string;
    value: string;
    invalid: boolean;
    onChange: (value: string) => void;
    onBlur: () => void;
}) {
    const size = Number(value);
    const step = (delta: number) => {
        const base = Number.isInteger(size) ? size : FONT_SIZE_MIN;
        onChange(String(Math.min(FONT_SIZE_MAX, Math.max(FONT_SIZE_MIN, base + delta))));
    };
    return (
        <ButtonGroup>
            <Button
                type='button'
                variant='outline'
                size='icon'
                aria-label='Smaller'
                disabled={size <= FONT_SIZE_MIN}
                onClick={() => step(-1)}
                className='h-8 w-8'
            >
                <Minus aria-hidden className='size-3.5' />
            </Button>
            <Input
                id={id}
                inputMode='numeric'
                value={value}
                aria-invalid={invalid || undefined}
                aria-describedby={invalid ? fieldHelperId(id) : undefined}
                onChange={(event) => onChange(event.target.value)}
                onBlur={onBlur}
                className='h-8 w-12 text-center font-mono text-sm font-semibold'
            />
            <Button
                type='button'
                variant='outline'
                size='icon'
                aria-label='Larger'
                disabled={size >= FONT_SIZE_MAX}
                onClick={() => step(1)}
                className='h-8 w-8'
            >
                <Plus aria-hidden className='size-3.5' />
            </Button>
        </ButtonGroup>
    );
}
