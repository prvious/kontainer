package auth

import (
	"testing"
	"time"
)

func TestCreateAndParseToken(t *testing.T) {
	tokenStr, err := CreateToken(testSecret, testUser, time.Hour)
	if err != nil {
		t.Fatalf("CreateToken failed: %v", err)
	}
	if tokenStr == "" {
		t.Fatal("CreateToken returned empty string")
	}

	claims, err := ParseToken(testSecret, tokenStr)
	if err != nil {
		t.Fatalf("ParseToken failed: %v", err)
	}

	if claims.UserID != testUser.ID {
		t.Errorf("UserID = %q, want %q", claims.UserID, testUser.ID)
	}
	if claims.Name != testUser.Name {
		t.Errorf("Name = %q, want %q", claims.Name, testUser.Name)
	}
	if claims.Email != testUser.Email {
		t.Errorf("Email = %q, want %q", claims.Email, testUser.Email)
	}
	if claims.AvatarURL != testUser.AvatarURL {
		t.Errorf("AvatarURL = %q, want %q", claims.AvatarURL, testUser.AvatarURL)
	}
	if claims.Provider != testUser.Provider {
		t.Errorf("Provider = %q, want %q", claims.Provider, testUser.Provider)
	}
	if claims.Issuer != "devoper-agent" {
		t.Errorf("Issuer = %q, want %q", claims.Issuer, "devoper-agent")
	}
}

func TestParseToken_ExpiredToken(t *testing.T) {
	tokenStr, err := CreateToken(testSecret, testUser, -time.Hour)
	if err != nil {
		t.Fatalf("CreateToken failed: %v", err)
	}

	_, err = ParseToken(testSecret, tokenStr)
	if err == nil {
		t.Fatal("ParseToken should fail for expired token")
	}
}

func TestParseToken_WrongSecret(t *testing.T) {
	tokenStr, err := CreateToken(testSecret, testUser, time.Hour)
	if err != nil {
		t.Fatalf("CreateToken failed: %v", err)
	}

	_, err = ParseToken("wrong-secret", tokenStr)
	if err == nil {
		t.Fatal("ParseToken should fail with wrong secret")
	}
}

func TestParseToken_MalformedToken(t *testing.T) {
	_, err := ParseToken(testSecret, "not.a.jwt")
	if err == nil {
		t.Fatal("ParseToken should fail for malformed token")
	}
}

func TestParseToken_EmptyToken(t *testing.T) {
	_, err := ParseToken(testSecret, "")
	if err == nil {
		t.Fatal("ParseToken should fail for empty token")
	}
}

func TestCreateToken_DifferentTTL(t *testing.T) {
	tokenStr, err := CreateToken(testSecret, testUser, 5*time.Minute)
	if err != nil {
		t.Fatalf("CreateToken failed: %v", err)
	}

	claims, err := ParseToken(testSecret, tokenStr)
	if err != nil {
		t.Fatalf("ParseToken failed: %v", err)
	}

	expiresAt := claims.ExpiresAt.Time
	issuedAt := claims.IssuedAt.Time
	diff := expiresAt.Sub(issuedAt)

	if diff != 5*time.Minute {
		t.Errorf("TTL = %v, want %v", diff, 5*time.Minute)
	}
}
