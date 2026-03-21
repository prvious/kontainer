import { Search, SlidersHorizontal, ChevronDown } from "lucide-react";
import { Input } from "@/components/ui/input";

interface TableToolbarProps {
    searchValue: string;
    onSearchChange: (value: string) => void;
    searchPlaceholder?: string;
}

export function TableToolbar({ searchValue, onSearchChange, searchPlaceholder }: TableToolbarProps) {
    return (
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
                <span>Filters</span>
                <SlidersHorizontal className='size-2.5 text-muted-foreground' />
            </button>
            <button className='flex items-center gap-1.5 rounded border border-border px-3.5 py-[7px] font-mono text-[11px] font-medium text-text-secondary'>
                <span>Sort: Recent activity</span>
                <ChevronDown className='size-2.5 text-muted-foreground' />
            </button>
        </div>
    );
}
