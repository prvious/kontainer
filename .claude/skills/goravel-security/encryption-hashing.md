# Encryption & Hashing Reference

## Encryption

Goravel encrypts values using AES-256-GCM with GMAC signing via `facades.Crypt()`.

### Configuration

Set `APP_KEY` in `.env`. Generate it with:

```shell
./artisan key:generate
```

The key is read from `config/app.go` (`key` option).

### EncryptString

```go
secret, err := facades.Crypt().EncryptString("sensitive data")
```

Returns an encrypted, GMAC-signed string. Tampered values will fail decryption.

### DecryptString

```go
plaintext, err := facades.Crypt().DecryptString(secret)
```

Returns an error if the GMAC signature is invalid or decryption fails.

## Hashing

Goravel hashes passwords via `facades.Hash()`. Supports Argon2id (default) and Bcrypt.

### Configuration

Set the default driver in `config/hashing.go`:

```go
// config/hashing.go
"driver": config.Env("HASHING_DRIVER", "argon2id"),
```

Available drivers: `"argon2id"`, `"bcrypt"`.

### Make (hash a password)

```go
hashed, err := facades.Hash().Make(password)
```

### Check (verify password against hash)

```go
if facades.Hash().Check("plain-text", hashedPassword) {
    // match
}
```

### NeedsRehash (detect outdated hash parameters)

Use during authentication to transparently upgrade hashes when the hashing configuration changes:

```go
if facades.Hash().NeedsRehash(hashed) {
    hashed, _ = facades.Hash().Make("plain-text")
    // persist the new hash
}
```
