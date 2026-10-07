import { useState } from "react";
import { Minus, Plus } from "lucide-react";
import { Track } from "@/components/charts/tracks";
import { TooltipRich } from "@/components/kit/tooltips";
import { useService } from "@/hooks/use-services";
import type { ServiceView } from "@/types/services";
import { cn } from "@/lib/utils";

const MAX_REPLICAS = 1000;

/** A partial count: hovering fetches the service and names the newest task error. */
function PartialCount({ service }: { service: ServiceView }) {
    const [asked, setAsked] = useState(false);
    const detail = useService(asked ? service.id : null);
    const pending = service.desiredReplicas - service.runningReplicas;
    const reason = detail.data?.latestError ?? (detail.isFetching ? "Asking the manager…" : "Waiting for the scheduler to place them.");
    return (
        <TooltipRich title={`${pending} of ${service.desiredReplicas} tasks pending`} reason={reason}>
            <span
                tabIndex={0}
                onMouseEnter={() => setAsked(true)}
                onFocus={() => setAsked(true)}
                className='cursor-help font-semibold text-warning underline decoration-dotted underline-offset-2'
            >
                {service.runningReplicas}/{service.desiredReplicas}
            </span>
        </TooltipRich>
    );
}

/**
 * REPLICAS (04.S3): `running/desired`. On a replicated service a click turns it into a stepper;
 * Enter submits, Esc cancels. While a scale runs the cell shows a track from the operation.
 */
export function ReplicaCell({
    service,
    scaling,
    editing,
    writable,
    onEdit,
    onCancel,
    onSubmit,
}: {
    service: ServiceView;
    /** The in-flight scale's `{ desired, running }`, when there is one. */
    scaling: { desired: number; running: number } | null;
    editing: boolean;
    writable: boolean;
    onEdit: () => void;
    onCancel: () => void;
    onSubmit: (replicas: number) => void;
}) {
    if (scaling) {
        return (
            <span className='flex w-full items-center justify-end gap-2'>
                <span className='w-14'>
                    <Track value={scaling.running} max={Math.max(scaling.desired, 1)} label={`${scaling.running} of ${scaling.desired} tasks running`} />
                </span>
                <span className='text-warning'>
                    {scaling.running}/{scaling.desired}
                </span>
            </span>
        );
    }
    if (editing) return <Stepper initial={service.desiredReplicas} name={service.name} onCancel={onCancel} onSubmit={onSubmit} />;
    const partial = service.runningReplicas > 0 && service.runningReplicas < service.desiredReplicas;
    const count = partial ? (
        <PartialCount service={service} />
    ) : (
        <span className={cn(service.runningReplicas === 0 && service.desiredReplicas > 0 && "text-destructive")}>
            {service.runningReplicas}/{service.desiredReplicas}
        </span>
    );
    if (service.mode !== "replicated" || !writable) return <span className='block text-right'>{count}</span>;
    return (
        <span className='flex justify-end'>
            {partial ? (
                <span className='flex items-center gap-1.5'>
                    {count}
                    <button
                        type='button'
                        aria-label={`Scale ${service.name}`}
                        onClick={onEdit}
                        className='rounded-sm px-1 text-muted-foreground hover:bg-secondary hover:text-foreground'
                    >
                        ±
                    </button>
                </span>
            ) : (
                <button
                    type='button'
                    aria-label={`Scale ${service.name}`}
                    onClick={onEdit}
                    className='rounded-sm px-1.5 py-0.5 hover:bg-secondary hover:text-foreground'
                >
                    {count}
                </button>
            )}
        </span>
    );
}

function Stepper({ initial, name, onCancel, onSubmit }: { initial: number; name: string; onCancel: () => void; onSubmit: (replicas: number) => void }) {
    const [value, setValue] = useState(initial);
    const clamp = (next: number) => Math.min(MAX_REPLICAS, Math.max(0, Number.isFinite(next) ? Math.round(next) : 0));
    return (
        <span className='flex justify-end'>
            <span
                role='group'
                aria-label={`Replicas for ${name}`}
                className='flex h-7 items-center rounded-md border border-primary bg-muted'
                onKeyDown={(event) => {
                    if (event.key === "Enter") {
                        event.preventDefault();
                        onSubmit(value);
                    } else if (event.key === "Escape") {
                        event.preventDefault();
                        event.stopPropagation();
                        onCancel();
                    } else if (event.key === "ArrowUp") {
                        event.preventDefault();
                        setValue((current) => clamp(current + 1));
                    } else if (event.key === "ArrowDown") {
                        event.preventDefault();
                        setValue((current) => clamp(current - 1));
                    }
                }}
            >
                <button
                    type='button'
                    aria-label='Fewer replicas'
                    onClick={() => setValue((current) => clamp(current - 1))}
                    className='flex size-7 items-center justify-center text-muted-foreground hover:text-foreground'
                >
                    <Minus aria-hidden className='size-3' />
                </button>
                <input
                    autoFocus
                    aria-label='Replicas'
                    inputMode='numeric'
                    value={value}
                    onChange={(event) => setValue(clamp(Number(event.target.value.replace(/\D/g, "") || 0)))}
                    className='w-9 bg-transparent text-center font-mono text-2xs text-foreground outline-none'
                />
                <button
                    type='button'
                    aria-label='More replicas'
                    onClick={() => setValue((current) => clamp(current + 1))}
                    className='flex size-7 items-center justify-center text-muted-foreground hover:text-foreground'
                >
                    <Plus aria-hidden className='size-3' />
                </button>
            </span>
        </span>
    );
}
