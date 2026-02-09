package images

import (
	"context"

	"github.com/docker/docker/api/types/image"
)

type DockerClient interface {
	ImageList(ctx context.Context, opts image.ListOptions) ([]image.Summary, error)
}

type ListImagesAction struct {
	client DockerClient
}

func NewListImagesAction(client DockerClient) *ListImagesAction {
	return &ListImagesAction{client: client}
}

func (a *ListImagesAction) Execute(ctx context.Context, nodeID, hostname string) ([]ImageItem, error) {
	imgs, err := a.client.ImageList(ctx, image.ListOptions{All: false})
	if err != nil {
		return nil, err
	}

	items := make([]ImageItem, 0, len(imgs))
	for _, img := range imgs {
		id := img.ID
		if len(id) > 19 {
			// Strip "sha256:" prefix and take first 12 chars
			if len(id) > 7 && id[:7] == "sha256:" {
				id = id[7:]
			}
			if len(id) > 12 {
				id = id[:12]
			}
		}

		items = append(items, ImageItem{
			ID:       id,
			Tags:     img.RepoTags,
			Size:     img.Size,
			Created:  img.Created,
			NodeID:   nodeID,
			Hostname: hostname,
		})
	}

	return items, nil
}
