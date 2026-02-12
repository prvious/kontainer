export interface TaskItem {
    id: string;
    serviceId: string;
    serviceName: string;
    slot: number;
    nodeId: string;
    nodeName: string;
    state: string;
    desiredState: string;
    exitCode: number | null;
    error: string;
    timestamp: string;
    containerId: string;
    image: string;
}
