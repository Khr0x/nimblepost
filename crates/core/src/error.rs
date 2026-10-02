use std::{fmt, io, path::PathBuf};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkError {
    Connect,
    Redirect,
    Response,
    Transport,
}

/// Diagnostics intentionally omit request URLs, bodies and third-party error text.
#[derive(Debug)]
pub enum Error {
    Io {
        path: PathBuf,
        kind: io::ErrorKind,
    },
    Yaml {
        path: PathBuf,
        line: usize,
        column: usize,
    },
    Config {
        path: PathBuf,
        field: String,
        message: &'static str,
    },
    MissingVariable {
        name: String,
        field: String,
    },
    Cancelled,
    Timeout {
        timeout_ms: u64,
    },
    Network(NetworkError),
    BodyTooLarge {
        limit: usize,
    },
    Conflict {
        path: PathBuf,
        temporary: Option<PathBuf>,
    },
}

impl Error {
    pub(crate) fn config(
        path: impl Into<PathBuf>,
        field: impl Into<String>,
        message: &'static str,
    ) -> Self {
        Self::Config {
            path: path.into(),
            field: field.into(),
            message,
        }
    }

    pub(crate) fn io(path: impl Into<PathBuf>, error: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            kind: error.kind(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, kind } => {
                write!(f, "No se pudo acceder a {} ({kind:?})", path.display())
            }
            Self::Yaml { path, line, column } => {
                write!(f, "YAML inválido en {}:{line}:{column}", path.display())
            }
            Self::Config {
                path,
                field,
                message,
            } => write!(f, "{} · {field}: {message}", path.display()),
            Self::MissingVariable { name, field } => {
                write!(f, "Variable '{name}' ausente en {field}")
            }
            Self::Cancelled => f.write_str("Petición cancelada"),
            Self::Timeout { timeout_ms } => {
                write!(f, "La petición superó el timeout de {timeout_ms} ms")
            }
            Self::Network(NetworkError::Connect) => {
                f.write_str("No se pudo conectar con el servidor (DNS, TCP o TLS)")
            }
            Self::Network(NetworkError::Redirect) => {
                f.write_str("Redirección rechazada o límite de redirecciones superado")
            }
            Self::Network(NetworkError::Response) => {
                f.write_str("No se pudo completar la lectura de la respuesta")
            }
            Self::Network(NetworkError::Transport) => f.write_str("Falló el transporte HTTP"),
            Self::BodyTooLarge { limit } => write!(
                f,
                "La respuesta supera el límite en memoria de {limit} bytes"
            ),
            Self::Conflict { path, temporary } => {
                write!(
                    f,
                    "Conflicto: {} cambió o fue eliminado. Recarga antes de guardar",
                    path.display()
                )?;
                if let Some(file) = temporary {
                    write!(f, ". Borrador conservado en {}", file.display())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for Error {}
