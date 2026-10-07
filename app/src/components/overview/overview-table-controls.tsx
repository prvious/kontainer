import { ListFilter, Search } from "lucide-react";
import { DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuContent, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { STATE_FILTERS, type StateFilter } from "@/components/containers/container-search";

/** The Overview table's header controls from `O2CiT`: a search box and the ALL STATES filter. */
export function OverviewTableControls({
    search,
    onSearchChange,
    states,
    onStatesChange,
}: {
    search: string;
    onSearchChange: (search: string) => void;
    states: StateFilter[];
    onStatesChange: (states: StateFilter[]) => void;
}) {
    const label = states.length === 0 ? "ALL STATES" : states.length === 1 ? states[0].toUpperCase() : `${states.length} STATES`;
    return (
        <>
            <label className='flex h-7.25 w-50 items-center gap-1.5 rounded-md border border-border px-3 focus-within:border-ring'>
                <Search aria-hidden className='size-3 shrink-0 text-muted-foreground' />
                <Input
                    type='search'
                    aria-label='Filter containers'
                    placeholder='Filter containers…'
                    value={search}
                    onChange={(event) => onSearchChange(event.target.value)}
                    className='h-auto border-0 bg-transparent p-0 font-mono text-2xs shadow-none focus-visible:ring-0 dark:bg-transparent'
                />
            </label>
            <DropdownMenu>
                <DropdownMenuTrigger className='flex h-7.25 items-center gap-1.5 rounded-md border border-border px-3 font-mono text-3xs font-semibold tracking-[0.5px] text-secondary-foreground hover:bg-secondary'>
                    <ListFilter aria-hidden className='size-3 text-muted-foreground' />
                    {label}
                </DropdownMenuTrigger>
                <DropdownMenuContent align='end' className='min-w-36'>
                    {STATE_FILTERS.map((state) => (
                        <DropdownMenuCheckboxItem
                            key={state}
                            checked={states.includes(state)}
                            onCheckedChange={(checked) => onStatesChange(checked ? [...states, state] : states.filter((value) => value !== state))}
                            className='font-mono text-2xs'
                        >
                            {state}
                        </DropdownMenuCheckboxItem>
                    ))}
                </DropdownMenuContent>
            </DropdownMenu>
        </>
    );
}
