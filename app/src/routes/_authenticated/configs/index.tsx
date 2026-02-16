import { createFileRoute, Link } from "@tanstack/react-router";
import { useState } from "react";
import { useConfigs } from "@/hooks/use-configs";
import { useCreateConfig } from "@/hooks/use-create-config";
import { useRemoveConfig } from "@/hooks/use-remove-config";
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
        <div className='flex flex-col'>
            <div className='flex h-10 items-center gap-3 border-b border-border px-3'>
                <Skeleton className='h-4 w-16' />
                <Skeleton className='h-4 w-8 rounded-full' />
                <div className='flex-1' />
                <Skeleton className='h-6 w-32' />
                <Skeleton className='h-6 w-24' />
            </div>
            <div className='space-y-0'>
                {[1, 2, 3, 4, 5].map((i) => (
                    <div key={i} className='flex items-center gap-3 border-b border-border px-3 py-2 last:border-0'>
                        <Skeleton className='h-3 w-28' />
                        <Skeleton className='h-3 w-16' />
                        <Skeleton className='ml-auto h-3 w-14' />
                    </div>
                ))}
            </div>
        </div>
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
            <SheetTrigger render={<Button variant='ghost' size='sm' className='h-6 px-2 text-[11px]' />}>
                <Plus className='size-3' />
                Create
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
                <Trash2 className='size-3 text-destructive' />
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

    if (isLoading) {
        return <LoadingSkeleton />;
    }

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-xs text-destructive'>
                <AlertCircle className='size-4' />
                <p>Failed to load configs</p>
            </div>
        );
    }

    return (
        <div className='flex h-full flex-col'>
            <div className='flex h-10 shrink-0 items-center gap-3 border-b border-border px-3'>
                <h1 className='text-xs font-semibold'>Configs</h1>
                <span className='rounded-md bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground'>{configs?.length ?? 0}</span>
                <div className='flex-1' />
                <div className='relative'>
                    <Search className='absolute top-1/2 left-2 size-3 -translate-y-1/2 text-muted-foreground' />
                    <Input
                        placeholder='Filter configs...'
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                        className='h-6 w-48 rounded-sm border-border bg-transparent pl-7 text-[11px] focus-visible:ring-1'
                    />
                </div>
                <CreateConfigSheet />
            </div>
            <div className='flex-1 overflow-y-auto'>
                {filteredConfigs.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <FileText className='size-4' />
                        <p>{search ? "No configs match your filter" : "No configs found"}</p>
                    </div>
                ) : (
                    <Table>
                        <TableHeader>
                            <TableRow className='bg-muted/40 hover:bg-muted/40'>
                                <TableHead className='h-7 text-[11px]'>Name</TableHead>
                                <TableHead className='h-7 text-[11px]'>Labels</TableHead>
                                <TableHead className='h-7 text-[11px]'>Size</TableHead>
                                <TableHead className='h-7 text-[11px]'>Created</TableHead>
                                <TableHead className='h-7 w-8' />
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {filteredConfigs.map((config) => (
                                <TableRow key={config.id}>
                                    <TableCell className='py-1.5'>
                                        <Link
                                            to='/configs/$configId'
                                            params={{ configId: config.id }}
                                            className='text-xs font-medium text-primary hover:underline'
                                        >
                                            {config.name}
                                        </Link>
                                    </TableCell>
                                    <TableCell className='py-1.5'>
                                        {Object.keys(config.labels).length === 0 ? (
                                            <span className='text-[11px] text-muted-foreground'>-</span>
                                        ) : (
                                            <div className='flex flex-wrap gap-1'>
                                                {Object.entries(config.labels).map(([k, v]) => (
                                                    <Badge key={k} variant='secondary' className='h-4 px-1.5 text-[10px]'>
                                                        {k}={v}
                                                    </Badge>
                                                ))}
                                            </div>
                                        )}
                                    </TableCell>
                                    <TableCell className='py-1.5'>
                                        <span className='font-mono text-[11px] text-muted-foreground'>{formatBytes(config.dataSize)}</span>
                                    </TableCell>
                                    <TableCell className='py-1.5'>
                                        <span className='font-mono text-[11px] text-muted-foreground'>{formatRelativeTime(config.createdAt)}</span>
                                    </TableCell>
                                    <TableCell className='py-1.5'>
                                        <RemoveButton onConfirm={() => removeConfig.mutate(config.id)} />
                                    </TableCell>
                                </TableRow>
                            ))}
                        </TableBody>
                    </Table>
                )}
            </div>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/configs/")({
    component: ConfigsPage,
});
