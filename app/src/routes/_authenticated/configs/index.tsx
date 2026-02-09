import { createFileRoute, Link } from "@tanstack/react-router";
import { useState } from "react";
import { useConfigs } from "@/hooks/use-configs";
import { useCreateConfig } from "@/hooks/use-create-config";
import { useRemoveConfig } from "@/hooks/use-remove-config";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle, SheetTrigger } from "@/components/ui/sheet";
import {
    AlertDialog,
    AlertDialogAction,
    AlertDialogCancel,
    AlertDialogContent,
    AlertDialogDescription,
    AlertDialogFooter,
    AlertDialogHeader,
    AlertDialogTitle,
    AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { AlertCircle, FileText, Plus, Search, Trash2 } from "lucide-react";
import { CodeEditor } from "@/components/code-editor";

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

function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    const kb = bytes / 1024;
    if (kb < 1024) return `${kb.toFixed(1)} KB`;
    const mb = kb / 1024;
    return `${mb.toFixed(1)} MB`;
}

function LoadingSkeleton() {
    return (
        <Card>
            <CardContent className='p-0'>
                <div className='space-y-0'>
                    {[1, 2, 3, 4, 5].map((i) => (
                        <div key={i} className='flex items-center gap-4 border-b px-4 py-3 last:border-0'>
                            <Skeleton className='h-4 w-32' />
                            <Skeleton className='h-4 w-20' />
                            <Skeleton className='ml-auto h-4 w-16' />
                        </div>
                    ))}
                </div>
            </CardContent>
        </Card>
    );
}

function CreateConfigSheet() {
    const [open, setOpen] = useState(false);
    const [name, setName] = useState("");
    const [data, setData] = useState("");
    const createConfig = useCreateConfig();

    function handleSubmit(e: React.FormEvent) {
        e.preventDefault();
        createConfig.mutate(
            { name, data },
            {
                onSuccess: () => {
                    setName("");
                    setData("");
                    setOpen(false);
                },
            },
        );
    }

    return (
        <Sheet open={open} onOpenChange={setOpen}>
            <SheetTrigger render={<Button size='sm' />}>
                <Plus className='size-3.5' />
                Create Config
            </SheetTrigger>
            <SheetContent className='sm:max-w-[54rem]'>
                <SheetHeader>
                    <SheetTitle>Create Config</SheetTitle>
                    <SheetDescription>Configs store non-sensitive configuration data accessible to services.</SheetDescription>
                </SheetHeader>
                <form onSubmit={handleSubmit} className='flex flex-1 flex-col gap-4 overflow-y-auto px-4'>
                    <div className='grid gap-2'>
                        <Label htmlFor='config-name'>Name</Label>
                        <Input id='config-name' placeholder='nginx.conf' value={name} onChange={(e) => setName(e.target.value)} required />
                    </div>
                    <div className='flex min-h-0 flex-1 flex-col gap-2'>
                        <Label>Data</Label>
                        <CodeEditor value={data} onChange={setData} className='min-h-0 flex-1' minHeight='200px' fill />
                    </div>
                </form>
                <SheetFooter className='border-t border-border'>
                    <Button onClick={handleSubmit} disabled={createConfig.isPending || !name || !data}>
                        {createConfig.isPending ? "Creating..." : "Create"}
                    </Button>
                </SheetFooter>
            </SheetContent>
        </Sheet>
    );
}

function RemoveButton({ onConfirm }: { onConfirm: () => void }) {
    return (
        <AlertDialog>
            <AlertDialogTrigger render={<Button variant='ghost' size='icon-sm' />} onClick={(e: React.MouseEvent) => e.stopPropagation()}>
                <Trash2 className='size-3.5 text-destructive' />
            </AlertDialogTrigger>
            <AlertDialogContent>
                <AlertDialogHeader>
                    <AlertDialogTitle>Remove config?</AlertDialogTitle>
                    <AlertDialogDescription>This action cannot be undone. Any services using this config will lose access.</AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                    <AlertDialogCancel>Cancel</AlertDialogCancel>
                    <AlertDialogAction onClick={onConfirm} render={<Button variant='destructive' />}>
                        Remove
                    </AlertDialogAction>
                </AlertDialogFooter>
            </AlertDialogContent>
        </AlertDialog>
    );
}

function ConfigsPage() {
    const [search, setSearch] = useState("");
    const { data: configs, isLoading, error } = useConfigs();
    const removeConfig = useRemoveConfig();

    const filteredConfigs = (configs ?? []).filter((c) => c.name.toLowerCase().includes(search.toLowerCase()));

    return (
        <div className='p-6'>
            <div className='mb-6 flex items-center justify-between'>
                <div>
                    <h1 className='text-xl font-semibold tracking-tight'>Configs</h1>
                    <p className='mt-1 text-sm text-muted-foreground'>{configs?.length ?? 0} configs</p>
                </div>
                <div className='flex items-center gap-2'>
                    <div className='relative'>
                        <Search className='absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground' />
                        <Input placeholder='Search...' value={search} onChange={(e) => setSearch(e.target.value)} className='h-8 w-48 pl-8 text-sm' />
                    </div>
                    <CreateConfigSheet />
                </div>
            </div>

            {isLoading ? (
                <LoadingSkeleton />
            ) : error ? (
                <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                    <AlertCircle className='size-5' />
                    <p>Failed to load configs</p>
                </div>
            ) : filteredConfigs.length === 0 ? (
                <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-muted-foreground'>
                    <FileText className='size-5' />
                    <p>{search ? "No configs match your search" : "No configs found"}</p>
                </div>
            ) : (
                <Card>
                    <CardContent className='p-0'>
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead>Name</TableHead>
                                    <TableHead>Labels</TableHead>
                                    <TableHead>Size</TableHead>
                                    <TableHead>Created</TableHead>
                                    <TableHead className='w-10' />
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {filteredConfigs.map((config) => (
                                    <TableRow key={config.id}>
                                        <TableCell>
                                            <Link to='/configs/$configId' params={{ configId: config.id }} className='font-medium text-primary hover:underline'>
                                                {config.name}
                                            </Link>
                                        </TableCell>
                                        <TableCell>
                                            {Object.keys(config.labels).length === 0 ? (
                                                <span className='text-xs text-muted-foreground'>-</span>
                                            ) : (
                                                <div className='flex flex-wrap gap-1'>
                                                    {Object.entries(config.labels).map(([k, v]) => (
                                                        <Badge key={k} variant='secondary' className='text-[10px]'>
                                                            {k}={v}
                                                        </Badge>
                                                    ))}
                                                </div>
                                            )}
                                        </TableCell>
                                        <TableCell>
                                            <span className='font-mono text-xs text-muted-foreground'>{formatBytes(config.dataSize)}</span>
                                        </TableCell>
                                        <TableCell>
                                            <span className='text-xs text-muted-foreground'>{formatRelativeTime(config.createdAt)}</span>
                                        </TableCell>
                                        <TableCell>
                                            <RemoveButton onConfirm={() => removeConfig.mutate(config.id)} />
                                        </TableCell>
                                    </TableRow>
                                ))}
                            </TableBody>
                        </Table>
                    </CardContent>
                </Card>
            )}
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/configs/")({
    component: ConfigsPage,
});
