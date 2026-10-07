import { useId, useState, type ReactNode } from "react";
import { Trash2, TriangleAlert, type LucideIcon } from "lucide-react";
import { AlertDialog, AlertDialogContent, AlertDialogDescription, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { Input } from "@/components/ui/input";
import { ActionButton } from "@/components/kit/action-button";
import { useConnectivity } from "@/hooks/use-connectivity";
import { cn } from "@/lib/utils";

export interface ConfirmDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    title: string;
    description?: ReactNode;
    /** Warning box listing what the action touches ("In use by 2 containers: …"). */
    impact?: ReactNode;
    /** When set, the confirm button stays disabled at 40 % until this exact text is typed. */
    confirmationText?: string;
    confirmLabel: string;
    confirmIcon?: LucideIcon;
    /** `neutral` is the non-destructive variant: neutral tile and a primary confirm button. */
    tone?: "destructive" | "neutral";
    icon?: LucideIcon;
    /** The operation is running: the confirm button shows its pending state. */
    pending?: boolean;
    /**
     * Confirms a change to cluster state, so it pauses while the controller is unreachable. False
     * for choices that stay on this device, such as leaving a page with unsaved edits.
     */
    mutates?: boolean;
    onConfirm: () => void;
    children?: ReactNode;
}

/**
 * Confirm Dialog `KIuxe`. Cancel and Esc close it without side effects; nothing runs until the
 * confirm button is pressed.
 */
export function ConfirmDialog(props: ConfirmDialogProps) {
    return (
        <AlertDialog open={props.open} onOpenChange={props.onOpenChange}>
            <AlertDialogContent className='w-115 max-w-[calc(100vw-2rem)] gap-0 rounded-lg bg-popover p-0 ring-border data-[size=default]:max-w-115 data-[size=default]:sm:max-w-115'>
                {/* Mounted only while open, so the typed text resets every time the dialog opens. */}
                <ConfirmDialogBody {...props} />
            </AlertDialogContent>
        </AlertDialog>
    );
}

function ConfirmDialogBody({
    onOpenChange,
    title,
    description,
    impact,
    confirmationText,
    confirmLabel,
    confirmIcon = Trash2,
    tone = "destructive",
    icon: Icon = tone === "destructive" ? Trash2 : TriangleAlert,
    pending = false,
    mutates = true,
    onConfirm,
    children,
}: ConfirmDialogProps) {
    const inputId = useId();
    const [typed, setTyped] = useState("");
    const matches = confirmationText === undefined || typed === confirmationText;
    const { reachable: controllerReachable } = useConnectivity();
    const reachable = controllerReachable || !mutates;

    return (
        <form
            onSubmit={(event) => {
                event.preventDefault();
                if (matches && !pending && reachable) onConfirm();
            }}
        >
            <div className='flex gap-3.5 px-5 pt-5'>
                <div
                    className={cn(
                        "flex size-9 shrink-0 items-center justify-center rounded-md",
                        tone === "destructive" ? "bg-destructive-soft text-destructive" : "bg-secondary text-muted-foreground",
                    )}
                >
                    <Icon aria-hidden className='size-4' />
                </div>
                <div className='flex min-w-0 flex-col gap-1.5'>
                    <AlertDialogTitle className='font-sans text-base font-semibold text-foreground'>{title}</AlertDialogTitle>
                    {description && (
                        <AlertDialogDescription className='font-mono text-2xs leading-relaxed text-muted-foreground'>{description}</AlertDialogDescription>
                    )}
                </div>
            </div>
            {impact && (
                <div className='pt-4 pr-5 pl-17.5'>
                    <div className='flex gap-2.5 rounded-sm bg-warning-soft px-3 py-2.5 font-mono text-2xs leading-normal font-medium text-warning'>
                        <TriangleAlert aria-hidden className='mt-0.5 size-3.25 shrink-0' />
                        <div className='min-w-0'>{impact}</div>
                    </div>
                </div>
            )}
            {children && <div className='pt-4 pr-5 pl-17.5'>{children}</div>}
            {confirmationText !== undefined && (
                <div className='flex flex-col gap-2 pt-4 pr-5 pl-17.5'>
                    <label htmlFor={inputId} className='font-mono text-3xs font-medium tracking-[1px] text-muted-foreground uppercase'>
                        Type {confirmationText} to confirm
                    </label>
                    <Input
                        id={inputId}
                        autoFocus
                        autoComplete='off'
                        spellCheck={false}
                        value={typed}
                        onChange={(event) => setTyped(event.target.value)}
                        className='h-9 rounded-md bg-muted font-mono text-xs'
                    />
                </div>
            )}
            <div className='mt-5 flex items-center justify-end gap-2 border-t border-border px-5 py-3.5'>
                <ActionButton type='button' variant='secondary' className='h-7.75 px-3.5 text-2xs' onClick={() => onOpenChange(false)}>
                    CANCEL
                </ActionButton>
                <ActionButton
                    type='submit'
                    variant={tone === "destructive" ? "destructive" : "primary"}
                    className='h-7.75 px-3.5 text-2xs'
                    icon={confirmIcon}
                    disabled={!matches}
                    pending={pending}
                    mutates={mutates}
                >
                    {confirmLabel}
                </ActionButton>
            </div>
        </form>
    );
}
