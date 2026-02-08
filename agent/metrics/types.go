package metrics

type NodeMetrics struct {
	CPUs          int     `json:"cpus"`
	MemoryTotal   uint64  `json:"memoryTotal"`
	MemoryUsed    uint64  `json:"memoryUsed"`
	MemoryPercent float64 `json:"memoryPercent"`
	CPUPercent    float64 `json:"cpuPercent"`
	DiskTotal     uint64  `json:"diskTotal"`
	DiskUsed      uint64  `json:"diskUsed"`
	DiskPercent   float64 `json:"diskPercent"`
}

type ContainerMetrics struct {
	ContainerID   string  `json:"containerId"`
	Name          string  `json:"name"`
	ServiceName   string  `json:"serviceName"`
	CPUPercent    float64 `json:"cpuPercent"`
	MemoryUsed    uint64  `json:"memoryUsed"`
	MemoryLimit   uint64  `json:"memoryLimit"`
	MemoryPercent float64 `json:"memoryPercent"`
}

type AgentMetricsSnapshot struct {
	Timestamp  string             `json:"timestamp"`
	NodeID     string             `json:"nodeId"`
	Hostname   string             `json:"hostname"`
	Node       NodeMetrics        `json:"node"`
	Containers []ContainerMetrics `json:"containers"`
}

type ClusterMetricsResponse struct {
	Agents  []AgentMetricsSnapshot `json:"agents"`
	History []AgentMetricsSnapshot `json:"history"`
}
