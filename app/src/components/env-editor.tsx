import { useState } from "react";
import { useUpdateServiceEnv } from "@/hooks/use-update-service-env";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Loader2, Plus, Save, Trash2 } from "lucide-react";
import type { EnvVar } from "@/types/service-detail";

export function EnvEditor({ serviceId, initialEnv }: { serviceId: string; initialEnv: EnvVar[] }) {
    const [rows, setRows] = useState<EnvVar[]>(initialEnv);
    const [dirty, setDirty] = useState(false);
    const { mutate, isPending } = useUpdateServiceEnv(serviceId);

    function updateRow(index: number, field: "key" | "value", val: string) {
        setRows((prev) => prev.map((r, i) => (i === index ? { ...r, [field]: val } : r)));
        setDirty(true);
    }

    function removeRow(index: number) {
        setRows((prev) => prev.filter((_, i) => i !== index));
        setDirty(true);
    }

    function addRow() {
        setRows((prev) => [...prev, { key: "", value: "" }]);
        setDirty(true);
    }

    function handleSave() {
        const filtered = rows.filter((r) => r.key.trim() !== "");
        mutate(filtered, {
            onSuccess: () => setDirty(false),
        });
    }

    return (
        <div className='space-y-4'>
            {rows.length === 0 ? (
                <p className='text-sm text-muted-foreground'>No environment variables set</p>
            ) : (
                <Table>
                    <TableHeader>
                        <TableRow>
                            <TableHead>Key</TableHead>
                            <TableHead>Value</TableHead>
                            <TableHead className='w-10' />
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        {rows.map((env, i) => (
                            <TableRow key={i}>
                                <TableCell>
                                    <Input
                                        value={env.key}
                                        onChange={(e) => updateRow(i, "key", e.target.value)}
                                        placeholder='KEY'
                                        className='h-8 font-mono text-xs'
                                        aria-label={`Env key ${i}`}
                                    />
                                </TableCell>
                                <TableCell>
                                    <Input
                                        value={env.value}
                                        onChange={(e) => updateRow(i, "value", e.target.value)}
                                        placeholder='value'
                                        className='h-8 font-mono text-xs'
                                        aria-label={`Env value ${i}`}
                                    />
                                </TableCell>
                                <TableCell>
                                    <Button variant='ghost' size='sm' className='size-7 p-0' onClick={() => removeRow(i)} aria-label={`Remove env ${i}`}>
                                        <Trash2 className='size-3.5 text-muted-foreground' />
                                    </Button>
                                </TableCell>
                            </TableRow>
                        ))}
                    </TableBody>
                </Table>
            )}
            <div className='flex items-center gap-2'>
                <Button variant='outline' size='sm' onClick={addRow}>
                    <Plus className='size-3.5' /> Add Variable
                </Button>
                {dirty && (
                    <Button size='sm' onClick={handleSave} disabled={isPending}>
                        {isPending ? <Loader2 className='size-3.5 animate-spin' /> : <Save className='size-3.5' />}
                        Save Changes
                    </Button>
                )}
            </div>
        </div>
    );
}
