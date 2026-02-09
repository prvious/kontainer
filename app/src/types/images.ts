export interface ImageItem {
    id: string;
    tags: string[];
    size: number;
    created: number;
    nodeId: string;
    hostname: string;
}

export interface NodeImages {
    nodeId: string;
    hostname: string;
    images: ImageItem[];
}

export interface ClusterImagesResponse {
    nodes: NodeImages[];
}

export interface RemoveImageResponse {
    deleted: string[];
    untagged: string[];
}

export interface PruneImagesResponse {
    spaceReclaimed: number;
    imagesDeleted: number;
}
