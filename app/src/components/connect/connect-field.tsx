import { forwardRef, useState, type ComponentProps } from "react";
import { Eye, EyeOff, type LucideIcon } from "lucide-react";
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from "@/components/ui/input-group";
import { FormField } from "@/components/kit/form-field";
import { fieldHelperId } from "@/components/kit/field-ids";
import { cn } from "@/lib/utils";

type InputProps = Omit<ComponentProps<"input">, "id">;

/** An icon-led input on the Connect card; `secret` adds the eye toggle. */
export const ConnectField = forwardRef<
    HTMLInputElement,
    InputProps & { id: string; label: string; icon: LucideIcon; error?: string; helper?: string; secret?: boolean }
>(function ConnectField({ id, label, icon: Icon, error, helper, secret = false, className, ...props }, ref) {
    const [revealed, setRevealed] = useState(false);
    return (
        <FormField id={id} label={label} error={error} helper={helper}>
            <InputGroup className={cn("h-11 rounded-md border-border bg-muted dark:bg-muted", error && "border-destructive")}>
                <InputGroupAddon className='pl-3.5'>
                    <Icon aria-hidden className='size-3.5 text-muted-foreground' />
                </InputGroupAddon>
                <InputGroupInput
                    ref={ref}
                    id={id}
                    type={secret && !revealed ? "password" : secret ? "text" : props.type}
                    aria-invalid={error ? true : undefined}
                    aria-describedby={error || helper ? fieldHelperId(id) : undefined}
                    className={cn("font-mono text-sm", className)}
                    {...props}
                />
                {secret && (
                    <InputGroupAddon align='inline-end' className='pr-2'>
                        <InputGroupButton
                            size='icon-xs'
                            aria-label={revealed ? "Hide password" : "Show password"}
                            onClick={() => setRevealed((current) => !current)}
                        >
                            {revealed ? <Eye aria-hidden /> : <EyeOff aria-hidden />}
                        </InputGroupButton>
                    </InputGroupAddon>
                )}
            </InputGroup>
        </FormField>
    );
});
