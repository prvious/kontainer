import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import {
    ArrowRight,
    Box,
    Container,
    HardDrive,
    KeyRound,
    Layers,
    LayoutDashboard,
    LogOut,
    Moon,
    Network,
    RefreshCw,
    ScrollText,
    Server,
    Settings,
    FileText,
    Sun,
    Terminal,
    Image,
} from "lucide-react";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useCommandPalette } from "@/hooks/use-command-palette";
import { useServices } from "@/hooks/use-services";
import { useNodes } from "@/hooks/use-nodes";
import { useStacks } from "@/hooks/use-stacks";
import { useServiceTasks } from "@/hooks/use-service-tasks";
import { useTerminal } from "@/hooks/use-terminal";
import { useAppearance } from "@/hooks/use-appearance";
import { useAuth } from "@/hooks/use-auth";
import type { ServiceItem } from "@/types/services";
import { router } from "@/router";

type PalettePage = { kind: "root" } | { kind: "service-actions"; service: ServiceItem } | { kind: "container-select"; service: ServiceItem };

interface CommandItem {
    id: string;
    label: string;
    description?: string;
    icon: React.ReactNode;
    action: () => void;
    category: string;
}

export function CommandPalette() {
    const { open, setOpen, initialSearch, setInitialSearch } = useCommandPalette();
    const navigate = useNavigate();
    const queryClient = useQueryClient();
    const { resolvedScheme, setScheme } = useAppearance();
    const { agentUrl, token, logout } = useAuth();
    const { openExec } = useTerminal();

    const [search, setSearch] = useState("");
    const [pages, setPages] = useState<PalettePage[]>([{ kind: "root" }]);
    const [selectedIndex, setSelectedIndex] = useState(0);
    const inputRef = useRef<HTMLInputElement>(null);

    const currentPage = pages[pages.length - 1];

    // Data hooks
    const { data: services } = useServices();
    const { data: nodes } = useNodes();
    const { data: stacks } = useStacks();

    // Only fetch tasks when on container-select page
    const serviceIdForTasks = currentPage.kind === "container-select" ? currentPage.service.id : "";
    const { data: tasks } = useServiceTasks(serviceIdForTasks);

    // Reset state when dialog opens/closes
    useEffect(() => {
        if (open) {
            setSearch(initialSearch);
            setPages([{ kind: "root" }]);
            setSelectedIndex(0);
            if (initialSearch) setInitialSearch("");
        }
    }, [open, initialSearch, setInitialSearch]);

    // Global ⌘K / Ctrl+K and ⌘T / Ctrl+T listener
    useEffect(() => {
        function handleKeyDown(e: KeyboardEvent) {
            if ((e.metaKey || e.ctrlKey) && e.key === "k") {
                e.preventDefault();
                setOpen(!open);
            }
            if ((e.metaKey || e.ctrlKey) && e.key === "t") {
                e.preventDefault();
                if (open) {
                    // Palette already open — apply filter directly
                    setPages([{ kind: "root" }]);
                    setSearch("terminal");
                    setSelectedIndex(0);
                } else {
                    setInitialSearch("terminal");
                    setOpen(true);
                }
            }
        }
        document.addEventListener("keydown", handleKeyDown);
        return () => document.removeEventListener("keydown", handleKeyDown);
    }, [open, setOpen, setInitialSearch]);

    const close = useCallback(() => {
        setOpen(false);
    }, [setOpen]);

    const pushPage = useCallback((page: PalettePage) => {
        setPages((prev) => [...prev, page]);
        setSearch("");
        setSelectedIndex(0);
    }, []);

    const popPage = useCallback(() => {
        setPages((prev) => {
            if (prev.length <= 1) return prev;
            setSearch("");
            setSelectedIndex(0);
            return prev.slice(0, -1);
        });
    }, []);

    // Build command list based on current page
    const commands = useMemo((): CommandItem[] => {
        if (currentPage.kind === "root") {
            const items: CommandItem[] = [];

            // Quick Actions
            items.push({
                id: "toggle-theme",
                label: resolvedScheme === "dark" ? "Switch to light mode" : "Switch to dark mode",
                icon: resolvedScheme === "dark" ? <Sun className='size-4' /> : <Moon className='size-4' />,
                action: () => {
                    setScheme(resolvedScheme === "dark" ? "light" : "dark");
                    close();
                },
                category: "Quick Actions",
            });
            items.push({
                id: "open-settings",
                label: "Settings",
                icon: <Settings className='size-4' />,
                action: () => {
                    void navigate({ to: "/settings" });
                    close();
                },
                category: "Quick Actions",
            });
            items.push({
                id: "disconnect",
                label: "Disconnect",
                icon: <LogOut className='size-4' />,
                action: () => {
                    void logout().then(() => {
                        router.invalidate().finally(() => navigate({ to: "/login", search: { redirect: undefined } }));
                    });
                    close();
                },
                category: "Quick Actions",
            });

            // Navigation
            const navItems: { id: string; label: string; to: string; icon: React.ReactNode }[] = [
                { id: "nav-overview", label: "Overview", to: "/", icon: <LayoutDashboard className='size-4' /> },
                { id: "nav-services", label: "Services", to: "/services", icon: <Layers className='size-4' /> },
                { id: "nav-nodes", label: "Nodes", to: "/nodes", icon: <Server className='size-4' /> },
                { id: "nav-stacks", label: "Stacks", to: "/stacks", icon: <Box className='size-4' /> },
                { id: "nav-containers", label: "Containers", to: "/tasks", icon: <Container className='size-4' /> },
                { id: "nav-networks", label: "Networking", to: "/networks", icon: <Network className='size-4' /> },
                { id: "nav-volumes", label: "Volumes", to: "/volumes", icon: <HardDrive className='size-4' /> },
                { id: "nav-images", label: "Images", to: "/images", icon: <Image className='size-4' /> },
                { id: "nav-secrets", label: "Secrets", to: "/secrets", icon: <KeyRound className='size-4' /> },
                { id: "nav-configs", label: "Configs", to: "/configs", icon: <FileText className='size-4' /> },
                { id: "nav-logs", label: "Logs", to: "/logs", icon: <ScrollText className='size-4' /> },
            ];
            for (const nav of navItems) {
                items.push({
                    id: nav.id,
                    label: nav.label,
                    icon: nav.icon,
                    action: () => {
                        void navigate({ to: nav.to });
                        close();
                    },
                    category: "Navigation",
                });
            }

            // Services (dynamic)
            if (services) {
                for (const svc of services) {
                    items.push({
                        id: `svc-${svc.id}`,
                        label: svc.name,
                        description: `${svc.replicasRunning}/${svc.replicasDesired} replicas · ${svc.image}`,
                        icon: <Layers className='size-4' />,
                        action: () => pushPage({ kind: "service-actions", service: svc }),
                        category: "Services",
                    });
                }
            }

            // Stacks (dynamic)
            if (stacks) {
                for (const stack of stacks) {
                    items.push({
                        id: `stack-${stack.name}`,
                        label: stack.name,
                        description: `${stack.services} services · ${stack.runningReplicas}/${stack.desiredReplicas} replicas`,
                        icon: <Box className='size-4' />,
                        action: () => {
                            void navigate({ to: "/stacks/$stackName", params: { stackName: stack.name } });
                            close();
                        },
                        category: "Stacks",
                    });
                }
            }

            // Nodes (dynamic)
            if (nodes) {
                for (const node of nodes) {
                    items.push({
                        id: `node-${node.id}`,
                        label: node.hostname,
                        description: `${node.role}${node.leader ? " · leader" : ""} · ${node.status}`,
                        icon: <Server className='size-4' />,
                        action: () => {
                            void navigate({ to: "/nodes" });
                            close();
                        },
                        category: "Nodes",
                    });
                }
            }

            // Terminal (dynamic — one exec entry per service)
            if (services) {
                for (const svc of services) {
                    items.push({
                        id: `terminal-${svc.id}`,
                        label: `Open terminal in ${svc.name}`,
                        description: `${svc.replicasRunning}/${svc.replicasDesired} replicas running`,
                        icon: <Terminal className='size-4' />,
                        action: () => pushPage({ kind: "container-select", service: svc }),
                        category: "Terminal",
                    });
                }
            }

            return items;
        }

        if (currentPage.kind === "service-actions") {
            const service = currentPage.service;
            return [
                {
                    id: "svc-detail",
                    label: "View details",
                    icon: <Layers className='size-4' />,
                    action: () => {
                        void navigate({ to: "/services/$serviceId", params: { serviceId: service.id } });
                        close();
                    },
                    category: service.name,
                },
                {
                    id: "svc-logs",
                    label: "View logs",
                    icon: <ScrollText className='size-4' />,
                    action: () => {
                        void navigate({ to: "/services/$serviceId", params: { serviceId: service.id } });
                        close();
                    },
                    category: service.name,
                },
                {
                    id: "svc-terminal",
                    label: "Open terminal",
                    description: "Select a running container",
                    icon: <Terminal className='size-4' />,
                    action: () => pushPage({ kind: "container-select", service }),
                    category: service.name,
                },
                {
                    id: "svc-restart",
                    label: "Restart service",
                    icon: <RefreshCw className='size-4' />,
                    action: () => {
                        void invoke("restart_service", { url: agentUrl, token, serviceId: service.id }).then(() => {
                            void queryClient.invalidateQueries({ queryKey: ["services"] });
                            void queryClient.invalidateQueries({ queryKey: ["service-tasks", service.id] });
                        });
                        close();
                    },
                    category: service.name,
                },
            ];
        }

        if (currentPage.kind === "container-select") {
            const service = currentPage.service;
            if (!tasks) return [];

            const running = tasks.filter((t) => t.state === "running");
            if (running.length === 0) {
                return [
                    {
                        id: "no-containers",
                        label: "No running containers",
                        icon: <Container className='size-4' />,
                        action: () => popPage(),
                        category: service.name,
                    },
                ];
            }

            return running.map((task) => ({
                id: `task-${task.id}`,
                label: `Slot ${task.slot} on ${task.nodeName}`,
                description: task.containerId.slice(0, 12),
                icon: <Terminal className='size-4' />,
                action: () => {
                    openExec({
                        containerId: task.containerId,
                        nodeId: task.nodeId,
                        containerName: service.name,
                    });
                    close();
                },
                category: service.name,
            }));
        }

        return [];
    }, [
        currentPage,
        services,
        nodes,
        stacks,
        tasks,
        resolvedScheme,
        setScheme,
        close,
        navigate,
        logout,
        pushPage,
        popPage,
        openExec,
        agentUrl,
        token,
        queryClient,
    ]);

    // Filter commands by search
    const filtered = useMemo(() => {
        if (!search.trim()) return commands;
        const query = search.toLowerCase();
        return commands.filter(
            (cmd) => cmd.label.toLowerCase().includes(query) || cmd.description?.toLowerCase().includes(query) || cmd.category.toLowerCase().includes(query),
        );
    }, [commands, search]);

    // Group filtered results by category
    const grouped = useMemo(() => {
        const map = new Map<string, CommandItem[]>();
        for (const cmd of filtered) {
            const existing = map.get(cmd.category);
            if (existing) {
                existing.push(cmd);
            } else {
                map.set(cmd.category, [cmd]);
            }
        }
        return map;
    }, [filtered]);

    // Reset selected index when filtered results change
    useEffect(() => {
        setSelectedIndex(0);
    }, [filtered.length, search]);

    // Keyboard navigation
    function handleKeyDown(e: React.KeyboardEvent) {
        if (e.key === "ArrowDown") {
            e.preventDefault();
            setSelectedIndex((i) => (i + 1) % Math.max(filtered.length, 1));
        } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setSelectedIndex((i) => (i - 1 + Math.max(filtered.length, 1)) % Math.max(filtered.length, 1));
        } else if (e.key === "Enter") {
            e.preventDefault();
            if (filtered[selectedIndex]) {
                filtered[selectedIndex].action();
            }
        } else if (e.key === "Escape") {
            if (pages.length > 1) {
                e.preventDefault();
                e.stopPropagation();
                popPage();
            }
        } else if (e.key === "Backspace" && search === "" && pages.length > 1) {
            e.preventDefault();
            popPage();
        }
    }

    // Breadcrumb for drill-down pages
    const breadcrumb = useMemo(() => {
        if (pages.length <= 1) return null;
        const labels: string[] = [];
        for (const page of pages) {
            if (page.kind === "root") labels.push("Commands");
            else if (page.kind === "service-actions") labels.push(page.service.name);
            else if (page.kind === "container-select") labels.push("Select Container");
        }
        return labels;
    }, [pages]);

    // Flatten index counter for rendering
    let flatIndex = 0;

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogContent showCloseButton={false} className='top-[35%] gap-0 p-0 sm:max-w-lg' onKeyDown={handleKeyDown}>
                <DialogTitle className='sr-only'>Command Palette</DialogTitle>

                {/* Breadcrumb */}
                {breadcrumb && breadcrumb.length > 1 && (
                    <div className='flex items-center gap-1 border-b border-border px-3 py-2 text-xs text-muted-foreground'>
                        {breadcrumb.map((label, i) => (
                            <span key={i} className='flex items-center gap-1'>
                                {i > 0 && <ArrowRight className='size-3' />}
                                <span className={i === breadcrumb.length - 1 ? "text-foreground" : ""}>{label}</span>
                            </span>
                        ))}
                    </div>
                )}

                {/* Search input */}
                <div className='flex items-center border-b border-border px-3'>
                    <input
                        ref={inputRef}
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                        placeholder={currentPage.kind === "root" ? "Type a command or search..." : "Filter..."}
                        className='flex-1 bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground'
                        autoFocus
                    />
                    {pages.length > 1 && (
                        <button onClick={popPage} className='ml-2 rounded px-1.5 py-0.5 text-xs text-muted-foreground hover:text-foreground'>
                            Esc
                        </button>
                    )}
                </div>

                {/* Results */}
                <ScrollArea className='max-h-80 overflow-y-auto'>
                    <div className='p-1'>
                        {filtered.length === 0 && <div className='px-3 py-6 text-center text-sm text-muted-foreground'>No results found</div>}
                        {Array.from(grouped.entries()).map(([category, items]) => (
                            <div key={category}>
                                <div className='px-3 py-1.5 text-xs font-medium text-muted-foreground'>{category}</div>
                                {items.map((cmd) => {
                                    const idx = flatIndex++;
                                    const isSelected = idx === selectedIndex;
                                    return (
                                        <button
                                            key={cmd.id}
                                            data-selected={isSelected || undefined}
                                            className='flex w-full items-center gap-3 rounded-md px-3 py-2 text-left text-sm hover:bg-accent data-selected:bg-accent'
                                            onClick={cmd.action}
                                            onMouseEnter={() => setSelectedIndex(idx)}
                                        >
                                            <span className='shrink-0 text-muted-foreground'>{cmd.icon}</span>
                                            <span className='flex-1 truncate'>
                                                <span>{cmd.label}</span>
                                                {cmd.description && <span className='ml-2 text-xs text-muted-foreground'>{cmd.description}</span>}
                                            </span>
                                            {(cmd.id.startsWith("svc-") || cmd.id.startsWith("terminal-")) && currentPage.kind === "root" && (
                                                <ArrowRight className='size-3 shrink-0 text-muted-foreground' />
                                            )}
                                        </button>
                                    );
                                })}
                            </div>
                        ))}
                    </div>
                </ScrollArea>

                {/* Footer */}
                <div className='flex items-center gap-4 border-t border-border px-3 py-2 text-[11px] text-muted-foreground'>
                    <span>
                        <kbd className='rounded border border-border bg-muted px-1 py-0.5 font-mono'>↑↓</kbd> navigate
                    </span>
                    <span>
                        <kbd className='rounded border border-border bg-muted px-1 py-0.5 font-mono'>↵</kbd> select
                    </span>
                    <span>
                        <kbd className='rounded border border-border bg-muted px-1 py-0.5 font-mono'>esc</kbd> {pages.length > 1 ? "back" : "close"}
                    </span>
                </div>
            </DialogContent>
        </Dialog>
    );
}
