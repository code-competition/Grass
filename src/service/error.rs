use std::{
    error::Error,
    fmt::{Display, Formatter},
};

#[derive(Debug, Clone)]
pub enum ServiceError {
    CouldNotGetSocket,
    GameDoesNotExist,
    GameDoesNotMatch,
    AlreadyInGame,
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        #[allow(deprecated)]
        match self.cause() {
            Some(cause) => write!(fmt, "{}: {}", self.description(), cause),
            None => write!(fmt, "{}", self.description()),
        }
    }
}

impl std::error::Error for ServiceError {
    fn description(&self) -> &str {
        #[allow(deprecated)]
        match *self {
            _ => ""
        }
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        match *self {
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CriticalError {
    code: i32,
}

impl CriticalError {
    #[allow(dead_code)]
    pub fn new(code: i32) -> Self {
        CriticalError { code }
    }

    pub fn get_code(&self) -> i32 {
        self.code
    }
}

impl Display for CriticalError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "Critical error: {}", self.code)
    }
}

impl Error for CriticalError {
    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

unsafe impl Send for CriticalError {}
unsafe impl Sync for CriticalError {}
