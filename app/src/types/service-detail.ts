export interface EnvVar {
    key: string;
    value: string;
}

export interface MountInfo {
    type: string;
    source: string;
    target: string;
    readOnly: boolean;
}

export interface PortInfo {
    publishedPort: number;
    targetPort: number;
    protocol: string;
    publishMode: string;
}

export interface UpdateStatusInfo {
    state: string;
    startedAt: string;
    completedAt: string;
    message: string;
}

export interface ServiceDetail {
    id: string;
    name: string;
    image: string;
    stack: string;
    mode: string;
    replicas: number | null;
    createdAt: string;
    updatedAt: string;
    env: EnvVar[];
    mounts: MountInfo[];
    constraints: string[];
    ports: PortInfo[];
    updateStatus: UpdateStatusInfo | null;
}
