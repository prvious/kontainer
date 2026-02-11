export interface NodeTaskItem {
    id: string;
    serviceId: string;
    serviceName: string;
    slot: number;
    state: string;
    desiredState: string;
    error: string;
    timestamp: string;
    containerId: string;
}
