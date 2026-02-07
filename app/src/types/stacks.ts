export interface StackItem {
    name: string;
    services: number;
    runningReplicas: number;
    desiredReplicas: number;
    updatedAt: string;
}
