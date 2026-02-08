package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/filters"
	"github.com/docker/docker/api/types/network"
)

type NetworkServiceRef struct {
	ID   string `json:"id"`
	Name string `json:"name"`
}

type NetworkItem struct {
	ID         string              `json:"id"`
	Name       string              `json:"name"`
	Driver     string              `json:"driver"`
	Scope      string              `json:"scope"`
	Internal   bool                `json:"internal"`
	Attachable bool                `json:"attachable"`
	Ingress    bool                `json:"ingress"`
	Services   []NetworkServiceRef `json:"services"`
	CreatedAt  string              `json:"createdAt"`
}

type ListNetworksAction struct {
	client DockerClient
}

func NewListNetworksAction(client DockerClient) *ListNetworksAction {
	return &ListNetworksAction{client: client}
}

func (a *ListNetworksAction) Execute(ctx context.Context) ([]NetworkItem, error) {
	// Fetch overlay networks (swarm scope)
	nets, err := a.client.NetworkList(ctx, network.ListOptions{
		Filters: filters.NewArgs(filters.Arg("driver", "overlay")),
	})
	if err != nil {
		return nil, err
	}

	// Fetch services to cross-reference which services are attached to each network
	svcs, err := a.client.ServiceList(ctx, types.ServiceListOptions{})
	if err != nil {
		return nil, err
	}

	// Build map: network ID/name -> list of service refs
	netServices := make(map[string][]NetworkServiceRef)
	for _, svc := range svcs {
		for _, netAttach := range svc.Spec.TaskTemplate.Networks {
			ref := NetworkServiceRef{
				ID:   svc.ID,
				Name: svc.Spec.Name,
			}
			netServices[netAttach.Target] = append(netServices[netAttach.Target], ref)
		}
	}

	items := make([]NetworkItem, 0, len(nets))
	for _, net := range nets {
		attached := netServices[net.ID]
		// Also check by name in case Target references name instead of ID
		if byName, ok := netServices[net.Name]; ok {
			attached = append(attached, byName...)
		}
		if attached == nil {
			attached = []NetworkServiceRef{}
		}

		items = append(items, NetworkItem{
			ID:         net.ID,
			Name:       net.Name,
			Driver:     net.Driver,
			Scope:      net.Scope,
			Internal:   net.Internal,
			Attachable: net.Attachable,
			Ingress:    net.Ingress,
			Services:   attached,
			CreatedAt:  net.Created.UTC().Format("2006-01-02T15:04:05Z"),
		})
	}

	return items, nil
}

type ListNetworksHandler struct {
	action *ListNetworksAction
}

func NewListNetworksHandler(action *ListNetworksAction) *ListNetworksHandler {
	return &ListNetworksHandler{action: action}
}

func (h *ListNetworksHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	items, err := h.action.Execute(r.Context())
	if err != nil {
		log.Printf("list networks error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to list networks"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{"networks": items})
}
