import { Check, CircleAlert, CircleDot, RotateCcw } from "lucide-react";
import { ActionButton } from "@/components/kit/action-button";
import { ROW_LABELS, type SettingsRow } from "@/components/settings/settings-form";
import { cn } from "@/lib/utils";

const plural = (count: number, word: string) => `${count} ${word}${count === 1 ? "" : "s"}`;

/** The sticky bar under staged edits (08.S5): counts, SAVE (off while any error), DISCARD. */
export function PendingChangesBar({
    changes,
    invalid,
    saving,
    onSave,
    onDiscard,
}: {
    changes: number;
    invalid: SettingsRow[];
    saving: boolean;
    onSave: () => void;
    onDiscard: () => void;
}) {
    const Icon = invalid.length > 0 ? CircleAlert : CircleDot;
    const fix = invalid.map((row) => ROW_LABELS[row].toLowerCase()).join(", ");
    return (
        <div
            role='region'
            aria-label='Pending changes'
            className='sticky bottom-0 z-10 flex items-center gap-3 rounded-md border border-border bg-card px-4 py-3 shadow-lg'
        >
            <Icon aria-hidden className={cn("size-4 shrink-0", invalid.length > 0 ? "text-destructive" : "text-sidebar-accent-foreground")} />
            <div className='flex min-w-0 flex-1 flex-col gap-0.5'>
                <span className='font-mono text-xs font-semibold text-foreground'>
                    {plural(changes, "unsaved change")}
                    {invalid.length > 0 && ` · ${plural(invalid.length, "error")}`}
                </span>
                <span className='truncate font-mono text-2xs text-muted-foreground'>
                    {invalid.length > 0 ? `Fix ${fix} to save.` : "Nothing is applied until you save."}
                </span>
            </div>
            <ActionButton variant='outline' icon={RotateCcw} onClick={onDiscard} disabled={saving}>
                DISCARD
            </ActionButton>
            <ActionButton variant='primary' icon={Check} pending={saving} disabled={invalid.length > 0} onClick={onSave}>
                SAVE CHANGES
            </ActionButton>
        </div>
    );
}
