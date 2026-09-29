import { Fragment } from "react";
import { Link, type LinkProps } from "@tanstack/react-router";
import { ChevronRight } from "lucide-react";

export interface Crumb {
    label: string;
    /** Every crumb but the current page links back. */
    to?: LinkProps["to"];
}

/** Breadcrumb from `lXH8d`: `Settings › Integrations › Traefik`, the last crumb is the page. */
export function Breadcrumbs({ trail }: { trail: Crumb[] }) {
    return (
        <nav aria-label='Breadcrumb'>
            <ol className='flex items-center gap-2 font-mono text-2xs font-medium text-muted-foreground'>
                {trail.map((crumb, index) => (
                    <Fragment key={crumb.label}>
                        {index > 0 && (
                            <li role='presentation' aria-hidden>
                                <ChevronRight className='size-3' />
                            </li>
                        )}
                        <li>
                            {crumb.to && index < trail.length - 1 ? (
                                <Link to={crumb.to} className='hover:text-foreground'>
                                    {crumb.label}
                                </Link>
                            ) : (
                                <span aria-current='page' className='text-foreground'>
                                    {crumb.label}
                                </span>
                            )}
                        </li>
                    </Fragment>
                ))}
            </ol>
        </nav>
    );
}
