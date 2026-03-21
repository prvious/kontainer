import type { LucideIcon } from "lucide-react";

interface MetricBadge {
    label: string;
    color: string;
    textColor: string;
}

interface MetricCardProps {
    icon: LucideIcon;
    iconColor: string;
    label: string;
    value: string;
    subtitle?: string;
    bar?: { percent: number; color: string };
    badges?: MetricBadge[];
}

export function MetricCard({ icon: Icon, iconColor, label, value, subtitle, bar, badges }: MetricCardProps) {
    return (
        <div className='flex h-[117px] flex-col gap-2 border border-border bg-card p-4'>
            {/* Label */}
            <div className='flex items-center gap-1.5'>
                <Icon className={`size-3 ${iconColor}`} />
                <span className='font-mono text-[9px] font-bold tracking-[0.5px] text-muted-foreground'>{label}</span>
            </div>

            {/* Value + subtitle */}
            <div className='flex items-end gap-2'>
                <span className='text-[32px] leading-none font-bold tracking-tight text-foreground'>{value}</span>
                {subtitle && <span className='mb-1 font-mono text-[11px] tracking-[0.3px] text-muted-foreground'>{subtitle}</span>}
            </div>

            {/* Progress bar OR badges */}
            {bar && (
                <div className='h-1 w-full bg-bg-elevated'>
                    <div className={`h-full ${bar.color}`} style={{ width: `${Math.min(bar.percent, 100)}%` }} />
                </div>
            )}
            {badges && (
                <div className='flex gap-2'>
                    {badges.map((badge) => (
                        <span key={badge.label} className={`rounded px-1.5 py-0.5 font-mono text-[10px] font-medium ${badge.color} ${badge.textColor}`}>
                            {badge.label}
                        </span>
                    ))}
                </div>
            )}
        </div>
    );
}
