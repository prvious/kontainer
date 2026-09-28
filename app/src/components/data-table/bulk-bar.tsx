import { useEffect, type ReactNode } from "react";
import { SquareCheck } from "lucide-react";
import { Kbd } from "@/components/ui/kbd";

/**
 * The bar that replaces the toolbar while rows are selected (state `q5WTPa`): count, context,
 * an actions slot, CLEAR SELECTION, and Esc to deselect.
 */
export function BulkBar({
    count,
    total,
    context,
    onClear,
    children,
}: {
    count: number;
    total: number;
    context?: string;
    onClear: () => void;
    children: ReactNode;
}) {
    // Esc anywhere clears the selection while the bar is shown (a document-level key listener).
    useEffect(() => {
        function onKeyDown(event: KeyboardEvent) {
            if (event.key === "Escape" && !event.defaultPrevented) onClear();
        }
        document.addEventListener("keydown", onKeyDown);
        return () => document.removeEventListener("keydown", onKeyDown);
    }, [onClear]);

    return (
        <div role='toolbar' aria-label='Bulk actions' className='flex h-11 items-center gap-4 rounded-md border border-border bg-card px-4'>
            <SquareCheck aria-hidden className='size-3.5 text-primary' />
            <span className='font-mono text-xs font-semibold text-foreground'>{count} selected</span>
            <span className='font-mono text-2xs text-muted-foreground'>
                of {total}
                {context ? ` · ${context}` : ""}
            </span>
            <div className='flex items-center gap-2'>{children}</div>
            <div className='ml-auto flex items-center gap-2'>
                <button
                    type='button'
                    onClick={onClear}
                    className='font-mono text-3xs font-semibold tracking-[0.5px] text-secondary-foreground uppercase hover:text-foreground'
                >
                    Clear selection
                </button>
                <Kbd className='font-mono text-3xs'>Esc</Kbd>
            </div>
        </div>
    );
}
