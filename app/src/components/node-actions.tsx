import { useUpdateNodeAvailability } from "@/hooks/use-update-node-availability";
import { useUpdateNodeRole } from "@/hooks/use-update-node-role";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { ArrowDown, ArrowUp, Loader2, MoreHorizontal, Pause, Play, Power } from "lucide-react";
import type { NodeItem } from "@/types/nodes";

export function NodeActions({ node }: { node: NodeItem }) {
    const { mutate: updateAvailability, isPending: isAvailPending } = useUpdateNodeAvailability(node.id);
    const { mutate: updateRole, isPending: isRolePending } = useUpdateNodeRole(node.id);
    const isPending = isAvailPending || isRolePending;

    return (
        <DropdownMenu>
            <DropdownMenuTrigger
                render={(props) => (
                    <Button {...props} variant='ghost' size='sm' className='size-7 p-0' disabled={isPending} aria-label={`Actions for ${node.hostname}`}>
                        {isPending ? <Loader2 className='size-3.5 animate-spin' /> : <MoreHorizontal className='size-3.5' />}
                    </Button>
                )}
            />
            <DropdownMenuContent align='end'>
                {node.availability === "active" ? (
                    <>
                        <DropdownMenuItem onClick={() => updateAvailability("drain")}>
                            <Power className='size-3.5' /> Drain
                        </DropdownMenuItem>
                        <DropdownMenuItem onClick={() => updateAvailability("pause")}>
                            <Pause className='size-3.5' /> Pause
                        </DropdownMenuItem>
                    </>
                ) : (
                    <DropdownMenuItem onClick={() => updateAvailability("active")}>
                        <Play className='size-3.5' /> Activate
                    </DropdownMenuItem>
                )}
                <DropdownMenuSeparator />
                {node.role === "worker" ? (
                    <DropdownMenuItem onClick={() => updateRole("manager")}>
                        <ArrowUp className='size-3.5' /> Promote to Manager
                    </DropdownMenuItem>
                ) : (
                    <DropdownMenuItem onClick={() => updateRole("worker")} disabled={node.leader}>
                        <ArrowDown className='size-3.5' /> Demote to Worker
                    </DropdownMenuItem>
                )}
            </DropdownMenuContent>
        </DropdownMenu>
    );
}
