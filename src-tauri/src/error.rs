use serde::Serialize;

#[derive(Debug)]
pub enum Error {
    SQLiteError(rusqlite::Error),
    IOError(std::io::Error)
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Error::SQLiteError(err) => serializer.serialize_str(&format!("{:?}", err)),
            Error::IOError(err) => serializer.serialize_str(&format!("{:?}", err)),
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        Self::SQLiteError(value)
    }
}

impl From<std::io::Error> for Error{
    fn from(value: std::io::Error) -> Self {
        Self::IOError(value)
    }
}
