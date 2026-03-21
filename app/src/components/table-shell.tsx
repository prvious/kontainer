import { Search } from "lucide-react";
import { Input } from "@/components/ui/input";

interface TableShellProps {
    title: string;
    count: number;
    searchValue: string;
    onSearchChange: (value: string) => void;
    searchPlaceholder?: string;
    filterSlot?: React.ReactNode;
    children: React.ReactNode;
}

export function TableShell({ title, count, searchValue, onSearchChange, searchPlaceholder, filterSlot, children }: TableShellProps) {
    return (
        <div className='flex flex-1 flex-col overflow-hidden border border-border bg-card'>
            <div className='flex items-center justify-between border-b border-border px-4 py-3'>
                <div className='flex items-center gap-3'>
                    <span className='font-mono text-[11px] font-bold tracking-[0.5px] text-foreground'>{title}</span>
                    <span className='rounded bg-bg-elevated px-2 py-0.5 font-mono text-[10px] text-muted-foreground'>{count}</span>
                </div>
                <div className='flex items-center gap-2'>
                    <div className='relative'>
                        <Search className='absolute top-1/2 left-2.5 size-3 -translate-y-1/2 text-muted-foreground' />
                        <Input
                            placeholder={searchPlaceholder ?? "Search..."}
                            value={searchValue}
                            onChange={(e) => onSearchChange(e.target.value)}
                            className='h-7 w-[200px] bg-transparent pl-8 font-mono text-[11px]'
                        />
                    </div>
                    {filterSlot}
                </div>
            </div>
            <div className='flex-1 overflow-y-auto'>{children}</div>
        </div>
    );
}
