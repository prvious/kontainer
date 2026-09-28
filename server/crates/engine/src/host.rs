use std::{fmt, path::PathBuf, str::FromStr};

pub const DEFAULT_DOCKER_SOCKET: &str = "/var/run/docker.sock";

/// Where the Docker Engine API is reached. Production uses the local Unix
/// socket; `Tcp` exists for the docker-in-docker development cluster (ADR
/// 0004) and callers must refuse it outside development and test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockerHost {
    UnixSocket(PathBuf),
    Tcp { host: String, port: u16 },
}

impl DockerHost {
    #[must_use]
    pub const fn is_tcp(&self) -> bool {
        matches!(self, Self::Tcp { .. })
    }
}

impl Default for DockerHost {
    fn default() -> Self {
        Self::UnixSocket(DEFAULT_DOCKER_SOCKET.into())
    }
}

impl fmt::Display for DockerHost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnixSocket(path) => write!(formatter, "unix://{}", path.display()),
            Self::Tcp { host, port } => write!(formatter, "tcp://{host}:{port}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockerHostParseError;

impl fmt::Display for DockerHostParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Docker host must be unix:///absolute/path or tcp://host:port")
    }
}

impl std::error::Error for DockerHostParseError {}

impl FromStr for DockerHost {
    type Err = DockerHostParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        if let Some(path) = value.strip_prefix("unix://") {
            return unix_socket(path);
        }
        if value.starts_with('/') {
            return unix_socket(value);
        }
        let authority = value.strip_prefix("tcp://").ok_or(DockerHostParseError)?;
        let (host, port) = authority.rsplit_once(':').ok_or(DockerHostParseError)?;
        let valid_host = !host.is_empty()
            && host
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || ".-_[]:".contains(character));
        let port = port.parse::<u16>().map_err(|_| DockerHostParseError)?;
        if !valid_host || port == 0 {
            return Err(DockerHostParseError);
        }
        Ok(Self::Tcp {
            host: host.to_owned(),
            port,
        })
    }
}

fn unix_socket(path: &str) -> Result<DockerHost, DockerHostParseError> {
    if path.starts_with('/') && path.len() > 1 {
        Ok(DockerHost::UnixSocket(path.into()))
    } else {
        Err(DockerHostParseError)
    }
}

#[cfg(test)]
mod tests {
    use super::{DockerHost, DockerHostParseError};

    #[test]
    fn parses_unix_and_tcp_hosts() {
        assert_eq!(
            "unix:///var/run/docker.sock".parse(),
            Ok(DockerHost::UnixSocket("/var/run/docker.sock".into()))
        );
        assert_eq!(
            "/tmp/docker.sock".parse(),
            Ok(DockerHost::UnixSocket("/tmp/docker.sock".into()))
        );
        assert_eq!(
            "tcp://localhost:2375".parse(),
            Ok(DockerHost::Tcp {
                host: "localhost".to_owned(),
                port: 2375
            })
        );
        assert_eq!(
            "tcp://127.0.0.1:2376"
                .parse::<DockerHost>()
                .unwrap()
                .to_string(),
            "tcp://127.0.0.1:2376"
        );
        assert!(
            "tcp://localhost:2375"
                .parse::<DockerHost>()
                .unwrap()
                .is_tcp()
        );
        assert!(!DockerHost::default().is_tcp());
    }

    #[test]
    fn rejects_other_schemes_and_malformed_authorities() {
        for invalid in [
            "",
            "http://localhost:2375",
            "https://localhost:2375",
            "ssh://user@host",
            "npipe:////./pipe/docker_engine",
            "tcp://",
            "tcp://localhost",
            "tcp://:2375",
            "tcp://localhost:0",
            "tcp://localhost:99999",
            "tcp://localhost:2375/path",
            "tcp://user@localhost:2375",
            "unix://",
            "unix://relative.sock",
            "relative.sock",
        ] {
            assert_eq!(
                invalid.parse::<DockerHost>(),
                Err(DockerHostParseError),
                "{invalid}"
            );
        }
    }
}
