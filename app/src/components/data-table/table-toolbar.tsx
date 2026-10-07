import type { ReactNode } from "react";
import { ChevronDown, Search, SlidersHorizontal, X } from "lucide-react";
import { DropdownMenu, DropdownMenuContent, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Input } from "@/components/ui/input";

export interface FilterChip {
    key: string;
    label: string;
    onRemove: () => void;
}

export interface SortOption {
    value: string;
    label: string;
}

/**
 * Table Toolbar `w9xU5`: search box, "Filters (n)" popover, sort dropdown, and the chips row
 * with "Clear filters". State lives in the page (URL search params); this only renders it.
 */
export function TableToolbar({
    search,
    onSearchChange,
    searchPlaceholder,
    filters,
    chips = [],
    onClearFilters,
    sortOptions,
    sort,
    onSortChange,
}: {
    search: string;
    onSearchChange: (search: string) => void;
    searchPlaceholder: string;
    /** Popover body for the Filters button; the button is hidden without it. */
    filters?: ReactNode;
    chips?: FilterChip[];
    onClearFilters?: () => void;
    sortOptions?: SortOption[];
    sort?: string;
    onSortChange?: (sort: string) => void;
}) {
    const sortLabel = sortOptions?.find((option) => option.value === sort)?.label;
    return (
        <div className='flex flex-col gap-2'>
            <div className='flex items-center gap-4'>
                <label className='flex h-7.25 flex-1 items-center gap-1.5 rounded-md border border-border px-3 focus-within:border-ring'>
                    <Search aria-hidden className='size-3 shrink-0 text-muted-foreground' />
                    <Input
                        type='search'
                        aria-label={searchPlaceholder}
                        placeholder={searchPlaceholder}
                        value={search}
                        onChange={(event) => onSearchChange(event.target.value)}
                        className='h-auto border-0 bg-transparent p-0 font-mono text-2xs shadow-none focus-visible:ring-0 dark:bg-transparent'
                    />
                </label>
                {filters && (
                    <Popover>
                        <PopoverTrigger className='flex h-7.25 items-center gap-1.5 rounded-md border border-border px-3.5 font-mono text-2xs font-medium text-secondary-foreground hover:bg-secondary'>
                            Filters ({chips.length})
                            <SlidersHorizontal aria-hidden className='size-2.5 text-muted-foreground' />
                        </PopoverTrigger>
                        <PopoverContent align='end' className='w-72 border-border bg-popover p-3'>
                            {filters}
                        </PopoverContent>
                    </Popover>
                )}
                {sortOptions && onSortChange && (
                    <DropdownMenu>
                        <DropdownMenuTrigger className='flex h-7.25 items-center gap-1.5 rounded-md border border-border px-3.5 font-mono text-2xs font-medium text-secondary-foreground hover:bg-secondary'>
                            Sort: {sortLabel}
                            <ChevronDown aria-hidden className='size-2.5 text-muted-foreground' />
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align='end' className='min-w-48'>
                            <DropdownMenuRadioGroup value={sort} onValueChange={(value) => onSortChange(String(value))}>
                                {sortOptions.map((option) => (
                                    <DropdownMenuRadioItem key={option.value} value={option.value} className='font-mono text-2xs'>
                                        {option.label}
                                    </DropdownMenuRadioItem>
                                ))}
                            </DropdownMenuRadioGroup>
                        </DropdownMenuContent>
                    </DropdownMenu>
                )}
            </div>
            {chips.length > 0 && (
                <div className='flex flex-wrap items-center gap-2'>
                    {chips.map((chip) => (
                        <span
                            key={chip.key}
                            className='inline-flex items-center gap-1.5 rounded-sm border border-border bg-secondary px-2.5 py-1 font-mono text-3xs font-medium text-secondary-foreground'
                        >
                            {chip.label}
                            <button
                                type='button'
                                aria-label={`Remove filter ${chip.label}`}
                                onClick={chip.onRemove}
                                className='text-muted-foreground hover:text-foreground'
                            >
                                <X aria-hidden className='size-2.5' />
                            </button>
                        </span>
                    ))}
                    {onClearFilters && (
                        <button
                            type='button'
                            onClick={onClearFilters}
                            className='font-mono text-3xs font-semibold text-secondary-foreground hover:text-foreground'
                        >
                            Clear filters
                        </button>
                    )}
                </div>
            )}
        </div>
    );
}
