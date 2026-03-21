package metrics

import (
	"errors"
	"time"
)

var ErrNoRecentMetrics = errors.New("no recent metrics")

type AgentMetricsSnapshot struct {
	Timestamp  time.Time          `json:"timestamp"`
	NodeID     string             `json:"nodeId"`
	Hostname   string             `json:"hostname"`
	Node       NodeMetrics        `json:"node"`
	Containers []ContainerMetrics `json:"containers"`
}

type ClusterMetricsResponse struct {
	Agents  []AgentMetricsSnapshot `json:"agents"`
	History []AgentMetricsSnapshot `json:"history"`
}
