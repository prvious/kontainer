import { useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';
import { Plus, ScrollText, Search } from 'lucide-react';
import { PageHeader } from '@/components/page-header';
import { LogStreamTabs } from '@/components/logs/log-stream-tabs';
import type { LogStream } from '@/components/logs/log-stream-tabs';
import { LogPanel } from '@/components/logs/log-panel';
import { StreamPickerDialog } from '@/components/logs/stream-picker-dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

const TAB_COLORS = ['bg-accent-green', 'bg-accent-cyan', 'bg-accent-orange', 'bg-accent-purple', 'bg-accent-red'];

function LogsPage() {
    const [streams, setStreams] = useState<LogStream[]>([]);
    const [activeStreamId, setActiveStreamId] = useState<string | null>(null);
    const [pickerOpen, setPickerOpen] = useState(false);
    const [logFilter, setLogFilter] = useState('');

    function handleAddStream(service: { id: string; name: string }) {
        const newStream: LogStream = {
            id: crypto.randomUUID(),
            serviceId: service.id,
            serviceName: service.name,
            color: TAB_COLORS[streams.length % TAB_COLORS.length],
        };
        setStreams((prev) => [...prev, newStream]);
        setActiveStreamId(newStream.id);
        setPickerOpen(false);
    }

    function handleCloseStream(id: string) {
        setStreams((prev) => {
            const next = prev.filter((s) => s.id !== id);
            if (activeStreamId === id) {
                setActiveStreamId(next.length > 0 ? next[next.length - 1].id : null);
            }
            return next;
        });
    }

    function handleSelectTab(id: string) {
        setActiveStreamId(id);
    }

    const hasStreams = streams.length > 0;

    return (
        <div className='flex h-full flex-col gap-4 p-6'>
            <PageHeader title='LOGS' subtitle='// per-service log streams'>
                <div className='relative'>
                    <Search className='absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground' />
                    <Input
                        placeholder='Filter logs...'
                        value={logFilter}
                        onChange={(e) => setLogFilter(e.target.value)}
                        className='h-8 w-[200px] rounded-md bg-bg-elevated pl-9 font-mono text-xs'
                    />
                </div>
                <button
                    onClick={() => setPickerOpen(true)}
                    className='flex items-center gap-1.5 rounded-md border border-accent-green bg-accent-green-20 px-3 py-1.5 font-mono text-[11px] font-semibold text-accent-green'
                >
                    <Plus className='size-3.5' />
                    New Stream
                </button>
            </PageHeader>

            {hasStreams ? (
                <>
                    <LogStreamTabs
                        streams={streams}
                        activeStreamId={activeStreamId}
                        onSelect={handleSelectTab}
                        onClose={handleCloseStream}
                        onAdd={() => setPickerOpen(true)}
                    />
                    <div className='flex flex-1 flex-col overflow-hidden rounded-b-lg border-x border-b border-border'>
                        {streams.map((stream) => (
                            <LogPanel
                                key={stream.id}
                                serviceId={stream.serviceId}
                                visible={stream.id === activeStreamId}
                                logFilter={logFilter}
                            />
                        ))}
                    </div>
                </>
            ) : (
                <div className='flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground'>
                    <ScrollText className='size-8' />
                    <p className='font-mono text-sm'>No log streams active</p>
                    <p className='font-mono text-xs'>Click &quot;New Stream&quot; to start watching service logs</p>
                    <Button variant='outline' size='sm' onClick={() => setPickerOpen(true)} className='mt-2 gap-1.5'>
                        <Plus className='size-3.5' />
                        Add Stream
                    </Button>
                </div>
            )}

            <StreamPickerDialog
                open={pickerOpen}
                onOpenChange={setPickerOpen}
                onSelect={handleAddStream}
                excludeServiceIds={streams.map((s) => s.serviceId)}
            />
        </div>
    );
}

export const Route = createFileRoute('/_authenticated/logs/')({
    component: LogsPage,
});
