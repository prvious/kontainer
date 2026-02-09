package images

type ImageItem struct {
	ID       string   `json:"id"`
	Tags     []string `json:"tags"`
	Size     int64    `json:"size"`
	Created  int64    `json:"created"`
	NodeID   string   `json:"nodeId"`
	Hostname string   `json:"hostname"`
}

type NodeImages struct {
	NodeID   string      `json:"nodeId"`
	Hostname string      `json:"hostname"`
	Images   []ImageItem `json:"images"`
}

type ClusterImagesResponse struct {
	Nodes []NodeImages `json:"nodes"`
}

type RemoveImageResponse struct {
	Deleted  []string `json:"deleted"`
	Untagged []string `json:"untagged"`
}

type PruneImagesResponse struct {
	SpaceReclaimed uint64 `json:"spaceReclaimed"`
	ImagesDeleted  int    `json:"imagesDeleted"`
}
