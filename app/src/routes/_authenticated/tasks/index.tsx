import { createFileRoute, Link } from "@tanstack/react-router";
import { useTasks } from "@/hooks/use-tasks";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Activity, AlertCircle, ArrowUpRight, Search } from "lucide-react";
import { useState } from "react";
import type { TaskItem } from "@/types/tasks";

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

function getStateColor(state: string): string {
	switch (state.toLowerCase()) {
		case "running":
			return "bg-success";
		case "failed":
		case "rejected":
			return "bg-destructive";
		case "complete":
		case "shutdown":
			return "bg-muted-foreground";
		case "preparing":
		case "pending":
		case "assigned":
		case "accepted":
			return "bg-warning";
		default:
			return "bg-muted-foreground";
	}
}

function TasksTable({ tasks }: { tasks: TaskItem[] }) {
	return (
		<Table>
			<TableHeader>
				<TableRow>
					<TableHead className='w-8' />
					<TableHead>Service</TableHead>
					<TableHead>Node</TableHead>
					<TableHead>Image</TableHead>
					<TableHead>State</TableHead>
					<TableHead className='text-right'>Updated</TableHead>
				</TableRow>
			</TableHeader>
			<TableBody>
				{tasks.map((task) => (
					<TableRow key={task.id} className='group'>
						<TableCell>
							<span className={`size-2 rounded-full ${getStateColor(task.state)}`} title={task.state} />
						</TableCell>
						<TableCell>
							<Link
								to='/services/$serviceId'
								params={{ serviceId: task.serviceId }}
								className='inline-flex items-center gap-1.5 font-medium hover:underline'
							>
								{task.serviceName}
								<ArrowUpRight className='size-3 opacity-0 transition-opacity group-hover:opacity-100' />
							</Link>
						</TableCell>
						<TableCell>
							<span className='text-sm text-muted-foreground'>{task.nodeName}</span>
						</TableCell>
						<TableCell>
							<span className='font-mono text-xs text-muted-foreground'>{task.image}</span>
						</TableCell>
						<TableCell>
							<Badge variant={task.state.toLowerCase() === "running" ? "default" : "secondary"}>{task.state}</Badge>
						</TableCell>
						<TableCell className='text-right'>
							<span className='text-xs text-muted-foreground'>{formatRelativeTime(task.timestamp)}</span>
						</TableCell>
					</TableRow>
				))}
			</TableBody>
		</Table>
	);
}

function LoadingSkeleton() {
	return (
		<div className='p-6'>
			<Card>
				<CardContent className='p-0'>
					<div className='space-y-0'>
						{[1, 2, 3, 4, 5].map((i) => (
							<div key={i} className='flex items-center gap-4 border-b px-4 py-3 last:border-0'>
								<Skeleton className='size-2 rounded-full' />
								<Skeleton className='h-4 w-32' />
								<Skeleton className='h-4 w-24' />
								<Skeleton className='h-4 w-48' />
								<Skeleton className='ml-auto h-5 w-16' />
							</div>
						))}
					</div>
				</CardContent>
			</Card>
		</div>
	);
}

function TasksPage() {
	const { data: tasks, isLoading, error } = useTasks();
	const [search, setSearch] = useState("");

	const filtered = (tasks ?? []).filter(
		(task) =>
			task.serviceName.toLowerCase().includes(search.toLowerCase()) ||
			task.nodeName.toLowerCase().includes(search.toLowerCase()) ||
			task.image.toLowerCase().includes(search.toLowerCase()) ||
			task.state.toLowerCase().includes(search.toLowerCase()),
	);

	if (isLoading) {
		return <LoadingSkeleton />;
	}

	if (error) {
		return (
			<div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
				<AlertCircle className='size-5' />
				<p>Failed to load tasks</p>
			</div>
		);
	}

	return (
		<div className='p-6'>
			<div className='mb-6 flex items-center justify-between'>
				<div>
					<h1 className='text-xl font-semibold tracking-tight'>Tasks</h1>
					<p className='mt-1 text-sm text-muted-foreground'>{tasks?.length ?? 0} tasks</p>
				</div>
				<div className='relative'>
					<Search className='absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground' />
					<Input placeholder='Search tasks...' value={search} onChange={(e) => setSearch(e.target.value)} className='h-8 w-56 pl-8 text-sm' />
				</div>
			</div>
			<div>
				{filtered.length === 0 ? (
					<Card>
						<CardContent className='flex flex-col items-center justify-center gap-3 py-16'>
							<div className='flex size-12 items-center justify-center rounded-lg bg-primary/10'>
								<Activity className='size-6 text-primary' />
							</div>
							<div className='text-center'>
								<p className='text-sm font-medium'>{search ? "No tasks match your search" : "No tasks found"}</p>
								<p className='mt-1 text-xs text-muted-foreground'>
									{search ? "Try a different search term" : "Deployment tasks and operations will appear here"}
								</p>
							</div>
						</CardContent>
					</Card>
				) : (
					<Card>
						<CardContent className='p-0'>
							<TasksTable tasks={filtered} />
						</CardContent>
					</Card>
				)}
			</div>
		</div>
	);
}

export const Route = createFileRoute("/_authenticated/tasks/")({
	component: TasksPage,
});
