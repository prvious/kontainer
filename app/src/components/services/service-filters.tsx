import { Checkbox } from "@/components/ui/checkbox";
import { STATUS_FILTERS, type ServiceSearch } from "@/components/services/service-search";
import { SERVICE_MODES } from "@/types/services";

/** The Filters popover body: mode, stack, and status. */
export function ServiceFilters({ search, stacks, onChange }: { search: ServiceSearch; stacks: string[]; onChange: (next: ServiceSearch) => void }) {
    return (
        <div className='flex flex-col gap-3 font-mono text-2xs'>
            <fieldset className='flex flex-col gap-1.5'>
                <legend className='mb-1 text-3xs font-bold tracking-[1px] text-muted-foreground uppercase'>Mode</legend>
                {SERVICE_MODES.map((mode) => (
                    <label key={mode} className='flex items-center gap-2 text-secondary-foreground'>
                        <Checkbox
                            checked={search.mode === mode}
                            onCheckedChange={(checked) => onChange({ ...search, mode: checked === true ? mode : undefined })}
                        />
                        {mode}
                    </label>
                ))}
            </fieldset>
            <fieldset className='flex flex-col gap-1.5'>
                <legend className='mb-1 text-3xs font-bold tracking-[1px] text-muted-foreground uppercase'>Stack</legend>
                {stacks.map((stack) => (
                    <label key={stack} className='flex items-center gap-2 text-secondary-foreground'>
                        <Checkbox
                            checked={search.stack === stack}
                            onCheckedChange={(checked) => onChange({ ...search, stack: checked === true ? stack : undefined })}
                        />
                        {stack}
                    </label>
                ))}
            </fieldset>
            <fieldset className='flex flex-col gap-1.5'>
                <legend className='mb-1 text-3xs font-bold tracking-[1px] text-muted-foreground uppercase'>Status</legend>
                {STATUS_FILTERS.map((status) => (
                    <label key={status} className='flex items-center gap-2 text-secondary-foreground'>
                        <Checkbox
                            checked={search.status === status}
                            onCheckedChange={(checked) => onChange({ ...search, status: checked === true ? status : undefined })}
                        />
                        {status}
                    </label>
                ))}
            </fieldset>
        </div>
    );
}
