package cluster

import "time"

const (
	RoleLeader   = "leader"
	RoleFollower = "follower"
)

type AgentInfo struct {
	NodeID   string `json:"nodeId"`
	Hostname string `json:"hostname"`
	Address  string `json:"address"`
	Role     string `json:"role"`
	Status   string `json:"status"`
	LastSeen string `json:"lastSeen"`
}

func NewAgentInfo(nodeID, hostname, address, role, status string) AgentInfo {
	return AgentInfo{
		NodeID:   nodeID,
		Hostname: hostname,
		Address:  address,
		Role:     role,
		Status:   status,
		LastSeen: time.Now().UTC().Format(time.RFC3339),
	}
}
