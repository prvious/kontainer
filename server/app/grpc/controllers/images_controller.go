package controllers

import (
	"context"
	"os"

	pb "github.com/prvious/kontainer/server/proto/agent"
)

func (c *AgentController) ListImages(ctx context.Context, req *pb.ListImagesRequest) (*pb.ListImagesResponse, error) {
	images, err := c.orchestrator.ListImages(ctx)
	if err != nil {
		return nil, err
	}

	hostname, _ := os.Hostname()
	items := make([]*pb.ImageItem, 0, len(images))
	for _, img := range images {
		items = append(items, &pb.ImageItem{
			Id:      img.ID,
			Tags:    img.Tags,
			Size:    img.Size,
			Created: img.Created,
		})
	}

	return &pb.ListImagesResponse{
		NodeId:   c.nodeID,
		Hostname: hostname,
		Images:   items,
	}, nil
}

func (c *AgentController) RemoveImage(ctx context.Context, req *pb.RemoveImageRequest) (*pb.RemoveImageResponse, error) {
	result, err := c.orchestrator.RemoveImage(ctx, req.ImageId, req.Force)
	if err != nil {
		return nil, err
	}

	return &pb.RemoveImageResponse{
		Deleted:  result.Deleted,
		Untagged: result.Untagged,
	}, nil
}

func (c *AgentController) PruneImages(ctx context.Context, req *pb.PruneImagesRequest) (*pb.PruneImagesResponse, error) {
	result, err := c.orchestrator.PruneImages(ctx)
	if err != nil {
		return nil, err
	}

	return &pb.PruneImagesResponse{
		SpaceReclaimed: result.SpaceReclaimed,
		ImagesDeleted:  int32(result.ImagesDeleted),
	}, nil
}
