export interface NodeMetrics {
    cpus: number;
    memoryTotal: number;
    memoryUsed: number;
    memoryPercent: number;
    cpuPercent: number;
    diskTotal: number;
    diskUsed: number;
    diskPercent: number;
}

export interface ContainerMetrics {
    containerId: string;
    name: string;
    serviceName: string;
    cpuPercent: number;
    memoryUsed: number;
    memoryLimit: number;
    memoryPercent: number;
    createdAt: number;
}

export interface AgentMetricsSnapshot {
    timestamp: string;
    nodeId: string;
    hostname: string;
    node: NodeMetrics;
    containers: ContainerMetrics[];
}

export interface ClusterMetricsResponse {
    agents: AgentMetricsSnapshot[];
    history: AgentMetricsSnapshot[];
}
