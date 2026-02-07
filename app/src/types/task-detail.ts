export interface TaskEnvVar {
    key: string;
    value: string;
}

export interface TaskMountInfo {
    type: string;
    source: string;
    target: string;
    readOnly: boolean;
}

export interface TaskNetworkInfo {
    name: string;
    addresses: string[];
}

export interface TaskDetail {
    id: string;
    serviceId: string;
    slot: number;
    nodeId: string;
    nodeName: string;
    state: string;
    desiredState: string;
    error: string;
    image: string;
    containerId: string;
    pid: number;
    exitCode: number | null;
    createdAt: string;
    updatedAt: string;
    env: TaskEnvVar[];
    mounts: TaskMountInfo[];
    networks: TaskNetworkInfo[];
}
