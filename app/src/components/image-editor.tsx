import { useState } from "react";
import { useUpdateServiceImage } from "@/hooks/use-update-service-image";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Check, Loader2, Pencil, X } from "lucide-react";

export function ImageEditor({ serviceId, currentImage }: { serviceId: string; currentImage: string }) {
    const [editing, setEditing] = useState(false);
    const [value, setValue] = useState(currentImage);
    const { mutate, isPending } = useUpdateServiceImage(serviceId);

    function handleSave() {
        if (value.trim() === "" || value === currentImage) {
            setEditing(false);
            setValue(currentImage);
            return;
        }
        mutate(value.trim(), {
            onSuccess: () => setEditing(false),
        });
    }

    function handleCancel() {
        setEditing(false);
        setValue(currentImage);
    }

    if (!editing) {
        return (
            <div className='flex items-center gap-2'>
                <span className='truncate font-mono text-xs'>{currentImage}</span>
                <Button variant='ghost' size='sm' className='size-6 p-0' onClick={() => setEditing(true)} aria-label='Edit image'>
                    <Pencil className='size-3 text-muted-foreground' />
                </Button>
            </div>
        );
    }

    return (
        <div className='flex items-center gap-1.5'>
            <Input
                value={value}
                onChange={(e) => setValue(e.target.value)}
                onKeyDown={(e) => {
                    if (e.key === "Enter") handleSave();
                    if (e.key === "Escape") handleCancel();
                }}
                className='h-7 font-mono text-xs'
                disabled={isPending}
                aria-label='Image tag'
                autoFocus
            />
            <Button variant='ghost' size='sm' className='size-7 p-0' onClick={handleSave} disabled={isPending} aria-label='Save image'>
                {isPending ? <Loader2 className='size-3.5 animate-spin' /> : <Check className='size-3.5 text-success' />}
            </Button>
            <Button variant='ghost' size='sm' className='size-7 p-0' onClick={handleCancel} disabled={isPending} aria-label='Cancel edit'>
                <X className='size-3.5 text-muted-foreground' />
            </Button>
        </div>
    );
}
