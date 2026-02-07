export interface ServiceTaskItem {
    id: string;
    slot: number;
    nodeId: string;
    nodeName: string;
    state: string;
    desiredState: string;
    exitCode: number | null;
    error: string;
    timestamp: string;
    containerId: string;
}
