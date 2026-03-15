package websocket

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestClampTail(t *testing.T) {
	tests := []struct {
		name     string
		raw      string
		fallback string
		want     string
	}{
		{"empty uses fallback", "", "100", "100"},
		{"valid number", "50", "100", "50"},
		{"all is allowed", "all", "100", "all"},
		{"negative uses fallback", "-1", "100", "100"},
		{"zero uses fallback", "0", "100", "100"},
		{"over 10000 is clamped", "99999", "100", "10000"},
		{"exactly 10000", "10000", "100", "10000"},
		{"non-numeric uses fallback", "abc", "100", "100"},
		{"one is valid", "1", "100", "1"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := clampTail(tt.raw, tt.fallback)
			if got != tt.want {
				t.Errorf("clampTail(%q, %q) = %q, want %q", tt.raw, tt.fallback, got, tt.want)
			}
		})
	}
}

func TestParseUint(t *testing.T) {
	tests := []struct {
		name     string
		s        string
		fallback uint
		want     uint
	}{
		{"valid number", "120", 80, 120},
		{"empty uses fallback", "", 80, 80},
		{"negative uses fallback", "-1", 80, 80},
		{"non-numeric uses fallback", "abc", 80, 80},
		{"zero is valid", "0", 80, 0},
		{"large valid", "4294967295", 80, 4294967295},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := parseUint(tt.s, tt.fallback)
			if got != tt.want {
				t.Errorf("parseUint(%q, %d) = %d, want %d", tt.s, tt.fallback, got, tt.want)
			}
		})
	}
}

func TestExecStatusMsg_ExitCodeZeroIncluded(t *testing.T) {
	// Regression test: Code field must NOT use omitempty, so exit code 0 is included.
	msg := execStatusMsg{Type: "exit", Code: 0}
	data, err := json.Marshal(msg)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	s := string(data)
	if !strings.Contains(s, `"code":0`) {
		t.Errorf("JSON = %s, want to contain \"code\":0", s)
	}
}

func TestExecStatusMsg_ExitCodeNonZero(t *testing.T) {
	msg := execStatusMsg{Type: "exit", Code: 137}
	data, _ := json.Marshal(msg)
	s := string(data)
	if !strings.Contains(s, `"code":137`) {
		t.Errorf("JSON = %s, want to contain \"code\":137", s)
	}
}

func TestExecStatusMsg_ErrorMessage(t *testing.T) {
	msg := execStatusMsg{Type: "error", Message: "something broke"}
	data, _ := json.Marshal(msg)
	s := string(data)
	if !strings.Contains(s, `"message":"something broke"`) {
		t.Errorf("JSON = %s, want to contain error message", s)
	}
}

func TestIsClosedError(t *testing.T) {
	tests := []struct {
		name string
		err  error
		want bool
	}{
		{"nil", nil, false},
		{"unrelated error", errStr("connection refused"), false},
		{"closed network connection", errStr("use of closed network connection"), true},
		{"closed pipe", errStr("io: read/write on closed pipe"), true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := isClosedError(tt.err)
			if got != tt.want {
				t.Errorf("isClosedError(%v) = %v, want %v", tt.err, got, tt.want)
			}
		})
	}
}

type errStr string

func (e errStr) Error() string { return string(e) }

func TestValidContainerID(t *testing.T) {
	tests := []struct {
		name  string
		id    string
		valid bool
	}{
		{"short hex", "abc123def456", true},
		{"full hex", "abc123def456abc123def456abc123def456abc123def456abc123def456abcd1234", true},
		{"container name", "my-container.name_1", true},
		{"empty", "", false},
		{"starts with dot", ".foo", false},
		{"has slash", "foo/bar", false},
		{"has space", "foo bar", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := validContainerID.MatchString(tt.id)
			if got != tt.valid {
				t.Errorf("validContainerID.MatchString(%q) = %v, want %v", tt.id, got, tt.valid)
			}
		})
	}
}
