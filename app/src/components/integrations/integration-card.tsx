import type { ReactNode } from "react";
import { Link, type LinkProps } from "@tanstack/react-router";
import { ArrowRight, type LucideIcon } from "lucide-react";
import { LabelBadge } from "@/components/kit/badges";
import { SkeletonLines } from "@/components/kit/skeletons";
import { cn } from "@/lib/utils";

const CARD = "flex h-50 flex-col gap-5 rounded-md border border-border bg-card p-4";

function CardBody({ icon: Icon, name }: { icon: LucideIcon; name: string }) {
    return (
        <>
            <div className='flex items-center justify-between gap-3'>
                <div className='flex items-center gap-3'>
                    <div className='flex size-8.5 items-center justify-center rounded-md border border-border bg-secondary'>
                        <Icon aria-hidden className='size-4 text-secondary-foreground' />
                    </div>
                    <h2 className='font-mono text-sm font-semibold text-foreground'>{name}</h2>
                </div>
                <LabelBadge>NOT CONNECTED</LabelBadge>
            </div>
            <SkeletonLines lines={2} className='flex-1 pt-3' />
            <div aria-hidden className='flex items-center justify-between'>
                <div className='h-7.5 w-24 rounded-md bg-secondary' />
                <ArrowRight className='size-3.5 text-muted-foreground' />
            </div>
        </>
    );
}

/**
 * An integration card from `Xmpv8` in its deferred state (ADR 0010): the identity tile, name,
 * and NOT CONNECTED badge, with skeleton lines for the description and footer. Only a card with
 * a page (`to`) opens.
 */
export function IntegrationCard({ icon, name, to }: { icon: LucideIcon; name: string; to?: LinkProps["to"] }): ReactNode {
    if (!to) {
        return (
            <section aria-label={`${name} (not connected)`} className={CARD}>
                <CardBody icon={icon} name={name} />
            </section>
        );
    }
    return (
        <Link
            to={to}
            aria-label={`${name} (not connected)`}
            className={cn(CARD, "outline-none hover:border-muted-foreground focus-visible:ring-2 focus-visible:ring-ring/50")}
        >
            <CardBody icon={icon} name={name} />
        </Link>
    );
}
