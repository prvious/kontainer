package storage

import (
	"database/sql"
	"fmt"

	_ "modernc.org/sqlite"
)

type AppConfigStore struct {
	db *sql.DB
}

func NewAppConfigStore(dbPath string) (*AppConfigStore, error) {
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return nil, fmt.Errorf("open sqlite: %w", err)
	}

	if _, err := db.Exec("PRAGMA journal_mode=WAL"); err != nil {
		db.Close()
		return nil, fmt.Errorf("set WAL mode: %w", err)
	}

	schema := `CREATE TABLE IF NOT EXISTS app_config (key TEXT PRIMARY KEY, value TEXT NOT NULL)`
	if _, err := db.Exec(schema); err != nil {
		db.Close()
		return nil, fmt.Errorf("create schema: %w", err)
	}

	return &AppConfigStore{db: db}, nil
}

func (s *AppConfigStore) Get(key string) (string, bool, error) {
	var value string
	err := s.db.QueryRow("SELECT value FROM app_config WHERE key = ?", key).Scan(&value)
	if err == sql.ErrNoRows {
		return "", false, nil
	}
	if err != nil {
		return "", false, fmt.Errorf("get app config: %w", err)
	}
	return value, true, nil
}

func (s *AppConfigStore) Set(key, value string) error {
	_, err := s.db.Exec(
		"INSERT INTO app_config (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
		key, value,
	)
	return err
}

func (s *AppConfigStore) GetAll() (map[string]string, error) {
	rows, err := s.db.Query("SELECT key, value FROM app_config")
	if err != nil {
		return nil, fmt.Errorf("get all app config: %w", err)
	}
	defer rows.Close()

	result := make(map[string]string)
	for rows.Next() {
		var key, value string
		if err := rows.Scan(&key, &value); err != nil {
			return nil, fmt.Errorf("scan row: %w", err)
		}
		result[key] = value
	}
	return result, rows.Err()
}

func (s *AppConfigStore) SetAll(config map[string]string) error {
	tx, err := s.db.Begin()
	if err != nil {
		return fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	stmt, err := tx.Prepare("INSERT INTO app_config (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
	if err != nil {
		return fmt.Errorf("prepare stmt: %w", err)
	}
	defer stmt.Close()

	for key, value := range config {
		if _, err := stmt.Exec(key, value); err != nil {
			return fmt.Errorf("upsert %q: %w", key, err)
		}
	}

	return tx.Commit()
}

func (s *AppConfigStore) IsSetupComplete() bool {
	val, ok, err := s.Get("setup_complete")
	if err != nil || !ok {
		return false
	}
	return val == "true"
}

func (s *AppConfigStore) Close() error {
	return s.db.Close()
}
