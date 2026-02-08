package cluster

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"time"
)

type Client struct {
	httpClient     *http.Client
	internalSecret string
}

func NewClient(internalSecret string) *Client {
	return &Client{
		httpClient: &http.Client{
			Timeout: 10 * time.Second,
		},
		internalSecret: internalSecret,
	}
}

func (c *Client) Get(ctx context.Context, address, path string) (*http.Response, error) {
	url := fmt.Sprintf("http://%s%s", address, path)
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set(InternalSecretHeader, c.internalSecret)
	return c.httpClient.Do(req)
}

func (c *Client) Post(ctx context.Context, address, path string, body io.Reader) (*http.Response, error) {
	url := fmt.Sprintf("http://%s%s", address, path)
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, url, body)
	if err != nil {
		return nil, err
	}
	req.Header.Set(InternalSecretHeader, c.internalSecret)
	req.Header.Set("Content-Type", "application/json")
	return c.httpClient.Do(req)
}
