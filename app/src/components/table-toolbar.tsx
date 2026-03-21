import { Search, SlidersHorizontal, ChevronDown } from "lucide-react";
import { Input } from "@/components/ui/input";

interface TableToolbarProps {
    searchValue: string;
    onSearchChange: (value: string) => void;
    searchPlaceholder?: string;
}

export function TableToolbar({ searchValue, onSearchChange, searchPlaceholder }: TableToolbarProps) {
    return (
        <div className='flex flex-col gap-2'>
            {/* Row 1: Search + Filters + Sort */}
            <div className='flex items-center gap-4'>
                <div className='relative flex-1'>
                    <Search className='absolute top-1/2 left-3 size-3 -translate-y-1/2 text-muted-foreground' />
                    <Input
                        placeholder={searchPlaceholder ?? "Search..."}
                        value={searchValue}
                        onChange={(e) => onSearchChange(e.target.value)}
                        className='h-8 bg-transparent pl-9 font-mono text-[11px]'
                    />
                </div>
                <button className='flex items-center gap-1.5 rounded border border-border px-3.5 py-[7px] font-mono text-[11px] font-medium text-text-secondary'>
                    <span>Filters (3)</span>
                    <SlidersHorizontal className='size-2.5 text-muted-foreground' />
                </button>
                <button className='flex items-center gap-1.5 rounded border border-border px-3.5 py-[7px] font-mono text-[11px] font-medium text-text-secondary'>
                    <span>Sort: Recent activity</span>
                    <ChevronDown className='size-2.5 text-muted-foreground' />
                </button>
            </div>

            {/* Row 2: Filter chips (decorative/NOOP) */}
            <div className='flex items-center gap-2'>
                <span className='rounded border border-border bg-[#101010] px-2.5 py-1 font-mono text-[10px] font-medium text-text-secondary'>
                    status: running
                </span>
                <span className='rounded border border-border bg-[#101010] px-2.5 py-1 font-mono text-[10px] font-medium text-text-secondary'>
                    node: leader-1
                </span>
                <span className='rounded border border-border bg-[#101010] px-2.5 py-1 font-mono text-[10px] font-medium text-text-secondary'>
                    ports: published
                </span>
                <span className='font-mono text-[10px] font-semibold text-accent-cyan'>Clear filters</span>
            </div>
        </div>
    );
}
