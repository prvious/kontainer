import { useLayoutEffect, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowDown, Scissors } from "lucide-react";
import type { LogBufferSnapshot, LogRow } from "@/components/logs/log-buffer";
import { logTime } from "@/lib/time";
import { logLevel } from "@/types/logs";
import { cn } from "@/lib/utils";

const ROW_HEIGHT = 29;
/** Within this many pixels of the end counts as at the bottom. */
const BOTTOM_SLACK = 16;

const levelClass = {
    destructive: "border-destructive/40 bg-destructive-soft text-destructive",
    warning: "border-warning/40 bg-warning-soft text-warning",
    neutral: "border-border bg-secondary text-secondary-foreground",
} as const;

function Row({ row, wrap }: { row: LogRow; wrap: boolean }) {
    if (row.kind === "gap") {
        return (
            <div role='row' className='flex h-7.25 items-center justify-center gap-2 border-b border-border bg-warning-soft font-mono text-3xs text-warning'>
                <Scissors aria-hidden className='size-3' />
                {row.dropped} {row.dropped === 1 ? "line" : "lines"} dropped while the client caught up
            </div>
        );
    }
    const level = logLevel(row.line);
    return (
        <div role='row' className='flex min-h-7.25 items-start gap-5 border-b border-border px-4 py-1.5 font-mono text-2xs'>
            <span role='cell' className='w-24 shrink-0 text-muted-foreground'>
                {logTime(row.line.ts)}
            </span>
            <span role='cell' className='w-14 shrink-0'>
                {level && <span className={cn("rounded-sm border px-1.5 py-0.5 text-3xs font-bold", levelClass[level.tone])}>{level.label}</span>}
            </span>
            <span role='cell' className={cn("min-w-0 flex-1 text-foreground", wrap ? "break-all whitespace-pre-wrap" : "truncate whitespace-pre")}>
                {row.line.text}
                {row.line.truncated && <span className='ml-2 text-muted-foreground'>[truncated]</span>}
            </span>
        </div>
    );
}

/**
 * The log table: TIMESTAMP, LEVEL, MESSAGE, virtualized. It follows new lines while scrolled to
 * the bottom; scrolling up pauses, and a pill counts what arrived since with JUMP TO LATEST
 * (08.S3). "At the bottom" comes from scroll events; the one effect keeps the DOM scrolled to the
 * end while following.
 */
export function LogViewer({ snapshot, rows, wrap }: { snapshot: LogBufferSnapshot; rows: LogRow[]; wrap: boolean }) {
    const scroller = useRef<HTMLDivElement>(null);
    const [following, setFollowing] = useState(true);
    const [pausedAt, setPausedAt] = useState(0);
    // The virtualizer hands back functions the React Compiler can't memoize, so it skips this component.
    // eslint-disable-next-line react-hooks/incompatible-library
    const virtualizer = useVirtualizer({
        count: rows.length,
        getScrollElement: () => scroller.current,
        estimateSize: () => ROW_HEIGHT,
        getItemKey: (index) => rows[index].key,
        overscan: 20,
    });

    useLayoutEffect(() => {
        if (following && rows.length > 0) virtualizer.scrollToIndex(rows.length - 1, { align: "end" });
    }, [following, rows.length, virtualizer]);

    function onScroll() {
        const element = scroller.current;
        if (!element) return;
        const atBottom = element.scrollHeight - element.scrollTop - element.clientHeight <= BOTTOM_SLACK;
        if (atBottom !== following) {
            setFollowing(atBottom);
            if (!atBottom) setPausedAt(snapshot.received);
        }
    }

    const fresh = following ? 0 : snapshot.received - pausedAt;
    return (
        <div className='relative flex min-h-0 flex-1 flex-col'>
            <div role='table' aria-label='Log lines' className='flex min-h-0 flex-1 flex-col'>
                <div
                    role='row'
                    className='flex h-9 shrink-0 items-center gap-5 border-b border-border bg-secondary px-4 font-mono text-3xs font-bold tracking-[1.1px] text-muted-foreground'
                >
                    <span role='columnheader' className='w-24'>
                        TIMESTAMP
                    </span>
                    <span role='columnheader' className='w-14'>
                        LEVEL
                    </span>
                    <span role='columnheader'>MESSAGE</span>
                </div>
                {/* Focusable so the scrollback scrolls from the keyboard. */}
                <div
                    ref={scroller}
                    role='rowgroup'
                    tabIndex={0}
                    onScroll={onScroll}
                    className='min-h-0 flex-1 overflow-y-auto outline-none focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:ring-inset'
                >
                    <div className='relative w-full' style={{ height: virtualizer.getTotalSize() }}>
                        {virtualizer.getVirtualItems().map((item) => (
                            <div
                                key={item.key}
                                data-index={item.index}
                                ref={virtualizer.measureElement}
                                className='absolute top-0 left-0 w-full'
                                style={{ transform: `translateY(${item.start}px)` }}
                            >
                                <Row row={rows[item.index]} wrap={wrap} />
                            </div>
                        ))}
                    </div>
                </div>
            </div>
            {!following && (
                <div className='pointer-events-none absolute inset-x-0 bottom-4 flex justify-center'>
                    <div className='pointer-events-auto flex items-center gap-3 rounded-full border border-border bg-popover px-4 py-2 font-mono text-2xs shadow-lg'>
                        <ArrowDown aria-hidden className='size-3.5 text-muted-foreground' />
                        <span className='text-foreground'>
                            {fresh} new {fresh === 1 ? "line" : "lines"}
                        </span>
                        <button
                            type='button'
                            onClick={() => setFollowing(true)}
                            className='font-bold tracking-[0.5px] text-sidebar-accent-foreground hover:underline'
                        >
                            JUMP TO LATEST
                        </button>
                        <span className='text-3xs text-muted-foreground'>· paused</span>
                    </div>
                </div>
            )}
        </div>
    );
}
