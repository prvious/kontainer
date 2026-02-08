import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { useSecrets } from "@/hooks/use-secrets";
import { useConfigs } from "@/hooks/use-configs";
import { useCreateSecret } from "@/hooks/use-create-secret";
import { useCreateConfig } from "@/hooks/use-create-config";
import { useRemoveSecret } from "@/hooks/use-remove-secret";
import { useRemoveConfig } from "@/hooks/use-remove-config";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
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
import { AlertCircle, FileText, KeyRound, Plus, Search, Trash2 } from "lucide-react";
import type { SecretItem } from "@/types/secrets";
import type { ConfigItem } from "@/types/configs";

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

function CreateSecretDialog() {
    const [open, setOpen] = useState(false);
    const [name, setName] = useState("");
    const [data, setData] = useState("");
    const createSecret = useCreateSecret();

    function handleSubmit(e: React.FormEvent) {
        e.preventDefault();
        createSecret.mutate(
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
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogTrigger render={<Button size='sm' />}>
                <Plus className='size-3.5' />
                Create Secret
            </DialogTrigger>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>Create Secret</DialogTitle>
                    <DialogDescription>Secrets are encrypted and only accessible to services that have been granted access.</DialogDescription>
                </DialogHeader>
                <form onSubmit={handleSubmit} className='grid gap-4'>
                    <div className='grid gap-2'>
                        <Label htmlFor='secret-name'>Name</Label>
                        <Input id='secret-name' placeholder='my-secret' value={name} onChange={(e) => setName(e.target.value)} required />
                    </div>
                    <div className='grid gap-2'>
                        <Label htmlFor='secret-data'>Data</Label>
                        <Textarea id='secret-data' placeholder='Secret content...' value={data} onChange={(e) => setData(e.target.value)} rows={4} required />
                    </div>
                    <DialogFooter>
                        <Button type='submit' disabled={createSecret.isPending}>
                            {createSecret.isPending ? "Creating..." : "Create"}
                        </Button>
                    </DialogFooter>
                </form>
            </DialogContent>
        </Dialog>
    );
}

function CreateConfigDialog() {
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
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogTrigger render={<Button size='sm' />}>
                <Plus className='size-3.5' />
                Create Config
            </DialogTrigger>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>Create Config</DialogTitle>
                    <DialogDescription>Configs store non-sensitive configuration data accessible to services.</DialogDescription>
                </DialogHeader>
                <form onSubmit={handleSubmit} className='grid gap-4'>
                    <div className='grid gap-2'>
                        <Label htmlFor='config-name'>Name</Label>
                        <Input id='config-name' placeholder='nginx.conf' value={name} onChange={(e) => setName(e.target.value)} required />
                    </div>
                    <div className='grid gap-2'>
                        <Label htmlFor='config-data'>Data</Label>
                        <Textarea
                            id='config-data'
                            placeholder='Configuration content...'
                            value={data}
                            onChange={(e) => setData(e.target.value)}
                            rows={4}
                            required
                        />
                    </div>
                    <DialogFooter>
                        <Button type='submit' disabled={createConfig.isPending}>
                            {createConfig.isPending ? "Creating..." : "Create"}
                        </Button>
                    </DialogFooter>
                </form>
            </DialogContent>
        </Dialog>
    );
}

function RemoveButton({ onConfirm, label }: { onConfirm: () => void; label: string }) {
    return (
        <AlertDialog>
            <AlertDialogTrigger render={<Button variant='ghost' size='icon-sm' />}>
                <Trash2 className='size-3.5 text-destructive' />
            </AlertDialogTrigger>
            <AlertDialogContent>
                <AlertDialogHeader>
                    <AlertDialogTitle>Remove {label}?</AlertDialogTitle>
                    <AlertDialogDescription>
                        This action cannot be undone. Any services using this {label.toLowerCase()} will lose access.
                    </AlertDialogDescription>
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

function SecretsTable({ secrets, onRemove }: { secrets: SecretItem[]; onRemove: (id: string) => void }) {
    return (
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead>Name</TableHead>
                    <TableHead>Labels</TableHead>
                    <TableHead>Created</TableHead>
                    <TableHead className='w-10' />
                </TableRow>
            </TableHeader>
            <TableBody>
                {secrets.map((secret) => (
                    <TableRow key={secret.id}>
                        <TableCell className='font-medium'>{secret.name}</TableCell>
                        <TableCell>
                            {Object.keys(secret.labels).length === 0 ? (
                                <span className='text-xs text-muted-foreground'>-</span>
                            ) : (
                                <div className='flex flex-wrap gap-1'>
                                    {Object.entries(secret.labels).map(([k, v]) => (
                                        <Badge key={k} variant='secondary' className='text-[10px]'>
                                            {k}={v}
                                        </Badge>
                                    ))}
                                </div>
                            )}
                        </TableCell>
                        <TableCell>
                            <span className='text-xs text-muted-foreground'>{formatRelativeTime(secret.createdAt)}</span>
                        </TableCell>
                        <TableCell>
                            <RemoveButton onConfirm={() => onRemove(secret.id)} label='secret' />
                        </TableCell>
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}

function ConfigsTable({ configs, onRemove }: { configs: ConfigItem[]; onRemove: (id: string) => void }) {
    return (
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
                {configs.map((config) => (
                    <TableRow key={config.id}>
                        <TableCell className='font-medium'>{config.name}</TableCell>
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
                            <RemoveButton onConfirm={() => onRemove(config.id)} label='config' />
                        </TableCell>
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}

function SecretsConfigsPage() {
    const [activeTab, setActiveTab] = useState<string>("secrets");
    const [search, setSearch] = useState("");

    const { data: secrets, isLoading: secretsLoading, error: secretsError } = useSecrets();
    const { data: configs, isLoading: configsLoading, error: configsError } = useConfigs();
    const removeSecret = useRemoveSecret();
    const removeConfig = useRemoveConfig();

    const filteredSecrets = (secrets ?? []).filter((s) => s.name.toLowerCase().includes(search.toLowerCase()));
    const filteredConfigs = (configs ?? []).filter((c) => c.name.toLowerCase().includes(search.toLowerCase()));

    const count = activeTab === "secrets" ? (secrets?.length ?? 0) : (configs?.length ?? 0);

    return (
        <div className='p-6'>
            <div className='mb-6 flex items-center justify-between'>
                <div>
                    <h1 className='text-xl font-semibold tracking-tight'>Secrets & Configs</h1>
                    <p className='mt-1 text-sm text-muted-foreground'>
                        {count} {activeTab}
                    </p>
                </div>
                <div className='flex items-center gap-2'>
                    <div className='relative'>
                        <Search className='absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground' />
                        <Input placeholder='Search...' value={search} onChange={(e) => setSearch(e.target.value)} className='h-8 w-48 pl-8 text-sm' />
                    </div>
                    {activeTab === "secrets" ? <CreateSecretDialog /> : <CreateConfigDialog />}
                </div>
            </div>
            <div>
                <Tabs value={activeTab} onValueChange={(v) => setActiveTab(v as string)}>
                    <TabsList>
                        <TabsTrigger value='secrets'>
                            <KeyRound className='size-3.5' />
                            Secrets
                        </TabsTrigger>
                        <TabsTrigger value='configs'>
                            <FileText className='size-3.5' />
                            Configs
                        </TabsTrigger>
                    </TabsList>

                    <TabsContent value='secrets' className='mt-4'>
                        {secretsLoading ? (
                            <LoadingSkeleton />
                        ) : secretsError ? (
                            <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                                <AlertCircle className='size-5' />
                                <p>Failed to load secrets</p>
                            </div>
                        ) : filteredSecrets.length === 0 ? (
                            <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-muted-foreground'>
                                <KeyRound className='size-5' />
                                <p>{search ? "No secrets match your search" : "No secrets found"}</p>
                            </div>
                        ) : (
                            <Card>
                                <CardContent className='p-0'>
                                    <SecretsTable secrets={filteredSecrets} onRemove={(id) => removeSecret.mutate(id)} />
                                </CardContent>
                            </Card>
                        )}
                    </TabsContent>

                    <TabsContent value='configs' className='mt-4'>
                        {configsLoading ? (
                            <LoadingSkeleton />
                        ) : configsError ? (
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
                                    <ConfigsTable configs={filteredConfigs} onRemove={(id) => removeConfig.mutate(id)} />
                                </CardContent>
                            </Card>
                        )}
                    </TabsContent>
                </Tabs>
            </div>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/secrets-configs/")({
    component: SecretsConfigsPage,
});
