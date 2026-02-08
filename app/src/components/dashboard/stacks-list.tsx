import { Link } from "@tanstack/react-router";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ArrowRight, Layers } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { StackItem } from "@/types/stacks";

interface StacksListProps {
    stacks: StackItem[];
}

function formatRelativeTime(iso: string): string {
    const seconds = Math.floor((Date.now() - new Date(iso).getTime()) / 1000);
    if (seconds < 60) return "just now";
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
}

export function StacksList({ stacks }: StacksListProps) {
    return (
        <Card className='animate-fade-in-up' style={{ animationDelay: "400ms" }}>
            <CardHeader className='pb-3'>
                <div className='flex items-center justify-between'>
                    <div className='flex items-center gap-2'>
                        <h3 className='text-sm font-medium'>Stacks</h3>
                        <span className='rounded-full bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground'>{stacks.length}</span>
                    </div>
                    <Link to='/stacks'>
                        <Button variant='ghost' size='sm' className='h-auto gap-1 px-2 py-1 text-xs text-muted-foreground'>
                            View all <ArrowRight className='size-3' />
                        </Button>
                    </Link>
                </div>
            </CardHeader>
            <CardContent className='p-0'>
                {stacks.length === 0 ? (
                    <div className='py-8 text-center text-sm text-muted-foreground'>No stacks found</div>
                ) : (
                    <div className='divide-y divide-border'>
                        {stacks.map((stack) => {
                            const healthy = stack.runningReplicas >= stack.desiredReplicas && stack.desiredReplicas > 0;

                            return (
                                <div key={stack.name} className='flex items-center gap-3 px-4 py-2.5'>
                                    <span className={`size-2 shrink-0 rounded-full ${healthy ? "bg-primary" : 'animate-pulse-dot bg-destructive'}`} />
                                    <div className='flex-1 overflow-hidden'>
                                        <div className='flex items-center gap-2'>
                                            <Layers className='size-3.5 shrink-0 text-muted-foreground' />
                                            <span className='truncate text-sm font-medium'>{stack.name}</span>
                                        </div>
                                        <p className='text-[11px] text-muted-foreground'>
                                            {stack.services} service{stack.services !== 1 ? "s" : ""}
                                        </p>
                                    </div>
                                    <div className='flex shrink-0 items-center gap-3'>
                                        <Badge variant={healthy ? "default" : "destructive"} className='font-mono text-[10px]'>
                                            {stack.runningReplicas}/{stack.desiredReplicas}
                                        </Badge>
                                        <span className='text-[11px] text-muted-foreground'>{formatRelativeTime(stack.updatedAt)}</span>
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                )}
            </CardContent>
        </Card>
    );
}
