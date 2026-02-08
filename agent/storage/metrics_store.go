package storage

import (
	"database/sql"
	"fmt"
	"time"

	_ "modernc.org/sqlite"
)

// MetricsRow represents a single node metrics data point stored in SQLite.
type MetricsRow struct {
	Timestamp   string
	NodeID      string
	Hostname    string
	CPUs        int
	CPUPercent  float64
	MemUsed     uint64
	MemTotal    uint64
	MemPercent  float64
	DiskUsed    uint64
	DiskTotal   uint64
	DiskPercent float64
}

type MetricsStore struct {
	db        *sql.DB
	retention time.Duration
}

func NewMetricsStore(dbPath string, retention time.Duration) (*MetricsStore, error) {
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return nil, fmt.Errorf("open sqlite: %w", err)
	}

	// Enable WAL mode for better concurrent read/write performance
	if _, err := db.Exec("PRAGMA journal_mode=WAL"); err != nil {
		db.Close()
		return nil, fmt.Errorf("set WAL mode: %w", err)
	}

	schema := `
	CREATE TABLE IF NOT EXISTS node_metrics (
		id         INTEGER PRIMARY KEY AUTOINCREMENT,
		timestamp  TEXT    NOT NULL,
		node_id    TEXT    NOT NULL,
		hostname   TEXT    NOT NULL,
		cpus       INTEGER NOT NULL,
		cpu_pct    REAL    NOT NULL,
		mem_used   INTEGER NOT NULL,
		mem_total  INTEGER NOT NULL,
		mem_pct    REAL    NOT NULL,
		disk_used  INTEGER NOT NULL,
		disk_total INTEGER NOT NULL,
		disk_pct   REAL    NOT NULL
	);
	CREATE INDEX IF NOT EXISTS idx_nm_ts ON node_metrics(timestamp);
	CREATE INDEX IF NOT EXISTS idx_nm_node ON node_metrics(node_id);
	`
	if _, err := db.Exec(schema); err != nil {
		db.Close()
		return nil, fmt.Errorf("create schema: %w", err)
	}

	return &MetricsStore{db: db, retention: retention}, nil
}

func (s *MetricsStore) Insert(row MetricsRow) error {
	_, err := s.db.Exec(
		`INSERT INTO node_metrics (timestamp, node_id, hostname, cpus, cpu_pct, mem_used, mem_total, mem_pct, disk_used, disk_total, disk_pct)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		row.Timestamp,
		row.NodeID,
		row.Hostname,
		row.CPUs,
		row.CPUPercent,
		row.MemUsed,
		row.MemTotal,
		row.MemPercent,
		row.DiskUsed,
		row.DiskTotal,
		row.DiskPercent,
	)
	return err
}

func (s *MetricsStore) Query(since time.Time, nodeID string) ([]MetricsRow, error) {
	var rows *sql.Rows
	var err error

	sinceStr := since.UTC().Format(time.RFC3339)

	if nodeID != "" {
		rows, err = s.db.Query(
			`SELECT timestamp, node_id, hostname, cpus, cpu_pct, mem_used, mem_total, mem_pct, disk_used, disk_total, disk_pct
			 FROM node_metrics WHERE timestamp >= ? AND node_id = ? ORDER BY timestamp ASC`,
			sinceStr, nodeID,
		)
	} else {
		rows, err = s.db.Query(
			`SELECT timestamp, node_id, hostname, cpus, cpu_pct, mem_used, mem_total, mem_pct, disk_used, disk_total, disk_pct
			 FROM node_metrics WHERE timestamp >= ? ORDER BY timestamp ASC`,
			sinceStr,
		)
	}
	if err != nil {
		return nil, fmt.Errorf("query metrics: %w", err)
	}
	defer rows.Close()

	var results []MetricsRow
	for rows.Next() {
		var r MetricsRow
		err := rows.Scan(
			&r.Timestamp,
			&r.NodeID,
			&r.Hostname,
			&r.CPUs,
			&r.CPUPercent,
			&r.MemUsed,
			&r.MemTotal,
			&r.MemPercent,
			&r.DiskUsed,
			&r.DiskTotal,
			&r.DiskPercent,
		)
		if err != nil {
			return nil, fmt.Errorf("scan row: %w", err)
		}
		results = append(results, r)
	}
	return results, rows.Err()
}

func (s *MetricsStore) Prune() error {
	cutoff := time.Now().Add(-s.retention).UTC().Format(time.RFC3339)
	_, err := s.db.Exec("DELETE FROM node_metrics WHERE timestamp < ?", cutoff)
	return err
}

func (s *MetricsStore) Close() error {
	return s.db.Close()
}
