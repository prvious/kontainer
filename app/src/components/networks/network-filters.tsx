import { Checkbox } from "@/components/ui/checkbox";
import type { NetworkSearch } from "@/components/networks/network-search";
import { NETWORK_SCOPES } from "@/types/networks";

/** The Filters popover body: driver, scope, and node. */
export function NetworkFilters({
    search,
    drivers,
    nodes,
    onChange,
}: {
    search: NetworkSearch;
    drivers: string[];
    nodes: string[];
    onChange: (next: NetworkSearch) => void;
}) {
    return (
        <div className='flex flex-col gap-3 font-mono text-2xs'>
            <fieldset className='flex flex-col gap-1.5'>
                <legend className='mb-1 text-3xs font-bold tracking-[1px] text-muted-foreground uppercase'>Driver</legend>
                {drivers.map((driver) => (
                    <label key={driver} className='flex items-center gap-2 text-secondary-foreground'>
                        <Checkbox
                            checked={search.driver === driver}
                            onCheckedChange={(checked) => onChange({ ...search, driver: checked === true ? driver : undefined })}
                        />
                        {driver}
                    </label>
                ))}
            </fieldset>
            <fieldset className='flex flex-col gap-1.5'>
                <legend className='mb-1 text-3xs font-bold tracking-[1px] text-muted-foreground uppercase'>Scope</legend>
                {NETWORK_SCOPES.map((scope) => (
                    <label key={scope} className='flex items-center gap-2 text-secondary-foreground'>
                        <Checkbox
                            checked={search.scope === scope}
                            onCheckedChange={(checked) => onChange({ ...search, scope: checked === true ? scope : undefined })}
                        />
                        {scope}
                    </label>
                ))}
            </fieldset>
            <fieldset className='flex flex-col gap-1.5'>
                <legend className='mb-1 text-3xs font-bold tracking-[1px] text-muted-foreground uppercase'>Node</legend>
                {nodes.map((node) => (
                    <label key={node} className='flex items-center gap-2 text-secondary-foreground'>
                        <Checkbox
                            checked={search.node === node}
                            onCheckedChange={(checked) => onChange({ ...search, node: checked === true ? node : undefined })}
                        />
                        {node}
                    </label>
                ))}
            </fieldset>
        </div>
    );
}
