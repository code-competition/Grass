/* 
    Code taken from r2d2-redis crate on github
    r2d2-redis is licensed under MIT license
*/

use std::error;
use std::error::Error as _StdError;
use std::fmt;

use redis::ConnectionLike;

/// A unified enum of errors returned by redis::Client
#[derive(Debug)]
pub enum Error {
    /// A redis::RedisError
    Other(redis::RedisError),
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        #[allow(deprecated)]
        match self.cause() {
            Some(cause) => write!(fmt, "{}: {}", self.description(), cause),
            None => write!(fmt, "{}", self.description()),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        #[allow(deprecated)]
        match *self {
            Error::Other(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        match *self {
            Error::Other(ref err) =>
            {
                #[allow(deprecated)]
                err.cause()
            }
        }
    }
}

#[derive(Debug)]
pub struct RedisConnectionManager {
    connection_info: redis::ConnectionInfo,
}

impl RedisConnectionManager {
    /// Creates a new `RedisConnectionManager`.
    ///
    /// See `redis::Client::open` for a description of the parameter
    /// types.
    pub fn new<T: redis::IntoConnectionInfo>(
        params: T,
    ) -> Result<RedisConnectionManager, redis::RedisError> {
        Ok(RedisConnectionManager {
            connection_info: params.into_connection_info()?,
        })
    }
}

impl r2d2::ManageConnection for RedisConnectionManager {
    type Connection = redis::Connection;
    type Error = Error;

    fn connect(&self) -> Result<redis::Connection, Error> {
        match redis::Client::open(self.connection_info.clone()) {
            Ok(client) => client.get_connection().map_err(Error::Other),
            Err(err) => Err(Error::Other(err)),
        }
    }

    fn is_valid(&self, conn: &mut redis::Connection) -> Result<(), Error> {
        redis::cmd("PING").query(conn).map_err(Error::Other)
    }

    fn has_broken(&self, conn: &mut redis::Connection) -> bool {
        !conn.is_open()
    }
}
