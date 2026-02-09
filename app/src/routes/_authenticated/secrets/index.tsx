import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { useSecrets } from "@/hooks/use-secrets";
import { useCreateSecret } from "@/hooks/use-create-secret";
import { useRemoveSecret } from "@/hooks/use-remove-secret";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle, SheetTrigger } from "@/components/ui/sheet";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
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
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { AlertCircle, Copy, Dices, Info, KeyRound, Plus, RefreshCw, Search, Trash2 } from "lucide-react";
import { CodeEditor } from "@/components/code-editor";
import type { SecretItem } from "@/types/secrets";

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

const CHARSETS = {
    lowercase: "abcdefghijklmnopqrstuvwxyz",
    uppercase: "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    digits: "0123456789",
    symbols: "!@#$%^&*()-_=+[]{}|;:,.<>?",
};

function generateSecret(length: number, options: { uppercase: boolean; digits: boolean; symbols: boolean }): string {
    let charset = CHARSETS.lowercase;
    if (options.uppercase) charset += CHARSETS.uppercase;
    if (options.digits) charset += CHARSETS.digits;
    if (options.symbols) charset += CHARSETS.symbols;

    const array = new Uint32Array(length);
    crypto.getRandomValues(array);
    return Array.from(array, (v) => charset[v % charset.length]).join("");
}

function SecretGenerator({ onUse }: { onUse: (value: string) => void }) {
    const [length, setLength] = useState(32);
    const [uppercase, setUppercase] = useState(true);
    const [digits, setDigits] = useState(true);
    const [symbols, setSymbols] = useState(true);
    const [preview, setPreview] = useState(() => generateSecret(32, { uppercase: true, digits: true, symbols: true }));
    const [copied, setCopied] = useState(false);

    function regenerate() {
        setPreview(generateSecret(length, { uppercase, digits, symbols }));
        setCopied(false);
    }

    function handleLengthChange(val: string) {
        const n = Number(val);
        setLength(n);
        setPreview(generateSecret(n, { uppercase, digits, symbols }));
        setCopied(false);
    }

    function handleCopy() {
        navigator.clipboard.writeText(preview);
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
    }

    return (
        <div className='rounded-lg border border-border bg-muted/30 p-3'>
            <div className='mb-3 flex items-center gap-2 text-xs font-medium text-muted-foreground'>
                <Dices className='size-3.5' />
                Secret Generator
            </div>

            <div className='mb-3 flex items-center gap-2'>
                <code className='flex-1 truncate rounded-md bg-background px-3 py-2 font-mono text-xs'>{preview}</code>
                <Button type='button' variant='ghost' size='icon-sm' onClick={handleCopy} title='Copy'>
                    <Copy className='size-3.5' />
                </Button>
                <Button type='button' variant='ghost' size='icon-sm' onClick={regenerate} title='Regenerate'>
                    <RefreshCw className='size-3.5' />
                </Button>
            </div>

            {copied && <p className='mb-2 text-[11px] text-green-600'>Copied!</p>}

            <div className='mb-3 grid grid-cols-2 gap-x-4 gap-y-2 text-xs'>
                <div className='flex items-center justify-between'>
                    <span className='text-muted-foreground'>Length</span>
                    <Select value={String(length)} onValueChange={handleLengthChange}>
                        <SelectTrigger className='h-7 w-18 text-xs'>
                            <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                            {[16, 24, 32, 48, 64, 128].map((n) => (
                                <SelectItem key={n} value={String(n)}>
                                    {n}
                                </SelectItem>
                            ))}
                        </SelectContent>
                    </Select>
                </div>
                <div className='flex items-center justify-between'>
                    <span className='text-muted-foreground'>Uppercase</span>
                    <Switch
                        checked={uppercase}
                        onCheckedChange={(v) => {
                            setUppercase(v);
                            setPreview(generateSecret(length, { uppercase: v, digits, symbols }));
                        }}
                    />
                </div>
                <div className='flex items-center justify-between'>
                    <span className='text-muted-foreground'>Digits</span>
                    <Switch
                        checked={digits}
                        onCheckedChange={(v) => {
                            setDigits(v);
                            setPreview(generateSecret(length, { uppercase, digits: v, symbols }));
                        }}
                    />
                </div>
                <div className='flex items-center justify-between'>
                    <span className='text-muted-foreground'>Symbols</span>
                    <Switch
                        checked={symbols}
                        onCheckedChange={(v) => {
                            setSymbols(v);
                            setPreview(generateSecret(length, { uppercase, digits, symbols: v }));
                        }}
                    />
                </div>
            </div>

            <Button type='button' variant='secondary' size='sm' className='w-full' onClick={() => onUse(preview)}>
                Use generated secret
            </Button>
        </div>
    );
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

function CreateSecretSheet() {
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
        <Sheet open={open} onOpenChange={setOpen}>
            <SheetTrigger render={<Button size='sm' />}>
                <Plus className='size-3.5' />
                Create Secret
            </SheetTrigger>
            <SheetContent className='sm:max-w-[54rem]'>
                <SheetHeader>
                    <SheetTitle>Create Secret</SheetTitle>
                    <SheetDescription>Secrets are encrypted and only accessible to services that have been granted access.</SheetDescription>
                </SheetHeader>
                <form onSubmit={handleSubmit} className='flex flex-1 flex-col gap-4 overflow-y-auto px-4'>
                    <div className='grid gap-2'>
                        <Label htmlFor='secret-name'>Name</Label>
                        <Input id='secret-name' placeholder='my-secret' value={name} onChange={(e) => setName(e.target.value)} required />
                    </div>
                    <SecretGenerator onUse={setData} />
                    <div className='flex min-h-0 flex-1 flex-col gap-2'>
                        <Label>Data</Label>
                        <CodeEditor value={data} onChange={setData} className='min-h-0 flex-1' minHeight='150px' fill />
                    </div>
                </form>
                <SheetFooter className='border-t border-border'>
                    <Button onClick={handleSubmit} disabled={createSecret.isPending || !name || !data}>
                        {createSecret.isPending ? "Creating..." : "Create"}
                    </Button>
                </SheetFooter>
            </SheetContent>
        </Sheet>
    );
}

function MetadataDialog({ secret }: { secret: SecretItem }) {
    return (
        <Dialog>
            <DialogTrigger render={<Button variant='ghost' size='icon-sm' />}>
                <Info className='size-3.5 text-muted-foreground' />
            </DialogTrigger>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>{secret.name}</DialogTitle>
                    <DialogDescription>Secret metadata — data cannot be read after creation.</DialogDescription>
                </DialogHeader>
                <div className='grid gap-3 text-sm'>
                    <div>
                        <span className='text-muted-foreground'>ID</span>
                        <p className='font-mono text-xs'>{secret.id}</p>
                    </div>
                    <div>
                        <span className='text-muted-foreground'>Labels</span>
                        {Object.keys(secret.labels).length === 0 ? (
                            <p className='text-xs text-muted-foreground'>None</p>
                        ) : (
                            <div className='mt-1 flex flex-wrap gap-1'>
                                {Object.entries(secret.labels).map(([k, v]) => (
                                    <Badge key={k} variant='secondary' className='text-[10px]'>
                                        {k}={v}
                                    </Badge>
                                ))}
                            </div>
                        )}
                    </div>
                    <div>
                        <span className='text-muted-foreground'>Created</span>
                        <p className='text-xs'>{new Date(secret.createdAt).toLocaleString()}</p>
                    </div>
                    <div>
                        <span className='text-muted-foreground'>Updated</span>
                        <p className='text-xs'>{new Date(secret.updatedAt).toLocaleString()}</p>
                    </div>
                </div>
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

function SecretsPage() {
    const [search, setSearch] = useState("");
    const { data: secrets, isLoading, error } = useSecrets();
    const removeSecret = useRemoveSecret();

    const filteredSecrets = (secrets ?? []).filter((s) => s.name.toLowerCase().includes(search.toLowerCase()));

    return (
        <div className='p-6'>
            <div className='mb-6 flex items-center justify-between'>
                <div>
                    <h1 className='text-xl font-semibold tracking-tight'>Secrets</h1>
                    <p className='mt-1 text-sm text-muted-foreground'>{secrets?.length ?? 0} secrets</p>
                </div>
                <div className='flex items-center gap-2'>
                    <div className='relative'>
                        <Search className='absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground' />
                        <Input placeholder='Search...' value={search} onChange={(e) => setSearch(e.target.value)} className='h-8 w-48 pl-8 text-sm' />
                    </div>
                    <CreateSecretSheet />
                </div>
            </div>

            {isLoading ? (
                <LoadingSkeleton />
            ) : error ? (
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
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead>Name</TableHead>
                                    <TableHead>Labels</TableHead>
                                    <TableHead>Created</TableHead>
                                    <TableHead className='w-20' />
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {filteredSecrets.map((secret) => (
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
                                            <div className='flex items-center gap-1'>
                                                <MetadataDialog secret={secret} />
                                                <RemoveButton onConfirm={() => removeSecret.mutate(secret.id)} label='secret' />
                                            </div>
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

export const Route = createFileRoute("/_authenticated/secrets/")({
    component: SecretsPage,
});
