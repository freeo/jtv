use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{program} was not found on PATH; install it and run `jtv doctor`")]
    MissingProgram { program: &'static str },

    #[error("failed to run {program}: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },

    #[error("{program} failed with exit status {status}: {stderr}")]
    ProgramFailed {
        program: String,
        status: i32,
        stderr: String,
    },

    #[error("unable to parse Justfile JSON: {0}")]
    JustJson(#[from] serde_json::Error),

    #[error("unable to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("unable to write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid configuration in {path}: {message}")]
    Config { path: PathBuf, message: String },

    #[error("invalid Television session: {0}")]
    InvalidSession(String),

    #[error("invalid selection identifier: {0}")]
    InvalidSelection(String),

    #[error("operation cancelled")]
    Cancelled,

    #[error("{0}")]
    Message(String),
}

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Cancelled => 130,
            Self::ProgramFailed { status, .. } if *status != 0 => *status,
            _ => 1,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
