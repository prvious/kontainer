package metrics

import (
	"time"

	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/models"
)

type PruneMetricsAction struct {
	retention time.Duration
}

func NewPruneMetricsAction(retention time.Duration) *PruneMetricsAction {
	return &PruneMetricsAction{retention: retention}
}

func (a *PruneMetricsAction) Execute() error {
	cutoff := time.Now().Add(-a.retention).UTC()
	_, err := facades.Orm().Query().Where("timestamp < ?", cutoff).Delete(&models.Metric{})
	return err
}
