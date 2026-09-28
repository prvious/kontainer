import { Checkbox } from "@/components/ui/checkbox";
import type { VolumeSearch } from "@/components/volumes/volume-search";

/** The Filters popover body: node, driver, and attached. */
export function VolumeFilters({
    search,
    nodes,
    drivers,
    onChange,
}: {
    search: VolumeSearch;
    nodes: string[];
    drivers: string[];
    onChange: (next: VolumeSearch) => void;
}) {
    return (
        <div className='flex flex-col gap-3 font-mono text-2xs'>
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
                <legend className='mb-1 text-3xs font-bold tracking-[1px] text-muted-foreground uppercase'>Attached</legend>
                {(["yes", "no"] as const).map((attached) => (
                    <label key={attached} className='flex items-center gap-2 text-secondary-foreground'>
                        <Checkbox
                            checked={search.attached === attached}
                            onCheckedChange={(checked) => onChange({ ...search, attached: checked === true ? attached : undefined })}
                        />
                        {attached === "yes" ? "attached" : "unattached"}
                    </label>
                ))}
            </fieldset>
        </div>
    );
}
