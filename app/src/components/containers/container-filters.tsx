import { Checkbox } from "@/components/ui/checkbox";
import { STATE_FILTERS, type ContainerSearch, type StateFilter } from "@/components/containers/container-search";
import type { NodeOutcomeView } from "@/types/containers";

/** The Filters popover body: state, node, and published ports. */
export function ContainerFilters({
    search,
    nodes,
    networks,
    onChange,
}: {
    search: ContainerSearch;
    nodes: NodeOutcomeView[];
    networks: string[];
    onChange: (next: ContainerSearch) => void;
}) {
    function toggleState(state: StateFilter, checked: boolean) {
        const current = new Set(search.state ?? []);
        if (checked) current.add(state);
        else current.delete(state);
        onChange({ ...search, state: current.size > 0 ? [...current] : undefined });
    }

    return (
        <div className='flex flex-col gap-3 font-mono text-2xs'>
            <fieldset className='flex flex-col gap-1.5'>
                <legend className='mb-1 text-3xs font-bold tracking-[1px] text-muted-foreground uppercase'>Status</legend>
                {STATE_FILTERS.map((state) => (
                    <label key={state} className='flex items-center gap-2 text-secondary-foreground'>
                        <Checkbox checked={search.state?.includes(state) ?? false} onCheckedChange={(checked) => toggleState(state, checked === true)} />
                        {state}
                    </label>
                ))}
            </fieldset>
            <fieldset className='flex flex-col gap-1.5'>
                <legend className='mb-1 text-3xs font-bold tracking-[1px] text-muted-foreground uppercase'>Node</legend>
                {nodes.map((node) => (
                    <label key={node.nodeId} className='flex items-center gap-2 text-secondary-foreground'>
                        <Checkbox
                            checked={search.node === node.nodeId}
                            onCheckedChange={(checked) => onChange({ ...search, node: checked === true ? node.nodeId : undefined })}
                        />
                        {node.nodeId}
                    </label>
                ))}
            </fieldset>
            {networks.length > 0 && (
                <fieldset className='flex flex-col gap-1.5'>
                    <legend className='mb-1 text-3xs font-bold tracking-[1px] text-muted-foreground uppercase'>Network</legend>
                    {networks.map((network) => (
                        <label key={network} className='flex items-center gap-2 text-secondary-foreground'>
                            <Checkbox
                                checked={search.network === network}
                                onCheckedChange={(checked) => onChange({ ...search, network: checked === true ? network : undefined })}
                            />
                            {network}
                        </label>
                    ))}
                </fieldset>
            )}
            <label className='flex items-center gap-2 text-secondary-foreground'>
                <Checkbox
                    checked={search.ports === "published"}
                    onCheckedChange={(checked) => onChange({ ...search, ports: checked === true ? "published" : undefined })}
                />
                ports published
            </label>
        </div>
    );
}
