use http::{uri::Scheme, Uri};
use serde::Serialize;
use serde_with::DeserializeFromStr;
use std::{
    fmt::{self, Display},
    str::FromStr,
};

#[derive(Debug, Clone, PartialEq, DeserializeFromStr)]
pub struct Endpoint {
    scheme: String,
    host: String,
    port: u16,
}

impl Endpoint {
    pub fn localhost(port: u16) -> Self {
        Endpoint {
            scheme: "http".to_owned(),
            host: "localhost".to_owned(),
            port,
        }
    }

    pub fn to_authority(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl FromStr for Endpoint {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uri = Uri::from_str(s)?;
        Endpoint::try_from(uri)
    }
}

impl Serialize for Endpoint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl Default for Endpoint {
    fn default() -> Self {
        Self::localhost(80)
    }
}

impl Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}://{}:{}", self.scheme, self.host, self.port)
    }
}

impl TryFrom<Uri> for Endpoint {
    type Error = anyhow::Error;
    fn try_from(value: Uri) -> Result<Self, Self::Error> {
        let host = value
            .host()
            .ok_or(anyhow::anyhow!(
                "Cannot parse endpoint host from URI '{value:?}'"
            ))?
            .to_owned();
        if host.is_empty() {
            return Err(anyhow::anyhow!("No host part in URI '{value:?}'"));
        }
        Ok(Self {
            scheme: value.scheme().unwrap_or(&Scheme::HTTP).to_string(),
            host,
            port: value.port_u16().unwrap_or(80),
        })
    }
}

#[cfg(test)]
mod tests {
    use http::Uri;

    use super::Endpoint;

    #[test]
    fn test_localhost() {
        let endpoint = Endpoint::localhost(8765);
        assert_eq!(endpoint.scheme, "http");
        assert_eq!(endpoint.host, "localhost");
        assert_eq!(endpoint.port, 8765);
    }

    #[test]
    fn try_from_ok() {
        let uri = Uri::from_static("https://foobar.local:9999");
        let endpoint = Endpoint::try_from(uri).unwrap();
        assert_eq!(endpoint.scheme, "https");
        assert_eq!(endpoint.host, "foobar.local");
        assert_eq!(endpoint.port, 9999);
    }

    #[test]
    fn try_from_default() {
        let uri = Uri::from_static("foobar.local");
        let endpoint = Endpoint::try_from(uri).unwrap();
        assert_eq!(endpoint.scheme, "http");
        assert_eq!(endpoint.host, "foobar.local");
        assert_eq!(endpoint.port, 80);
    }

    #[test]
    fn try_from_err() {
        let uri = Uri::from_static("/:9999/abc");
        let err = Endpoint::try_from(uri).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Cannot parse endpoint host from URI '/:9999/abc'"
        );

        let uri = Uri::from_static("http://:9999/abc");
        let err = Endpoint::try_from(uri).unwrap_err();
        assert_eq!(err.to_string(), "No host part in URI 'http://:9999/abc'");
    }

    #[test]
    fn to_authority() {
        let endpoint = Endpoint::localhost(8765);
        assert_eq!(endpoint.to_authority(), "localhost:8765");
    }

    #[test]
    fn test_to_string() {
        let endpoint = Endpoint::localhost(8765);
        assert!(endpoint.to_string().contains("http://localhost:8765"));
    }

    #[test]
    fn serialize() {
        let endpoint = Endpoint::localhost(8765);
        assert_eq!(
            serde_json::to_string(&endpoint).unwrap(),
            "\"http://localhost:8765\""
        )
    }

    #[test]
    fn deserialize() {
        assert_eq!(
            serde_json::from_str::<Endpoint>("\"http://localhost:8765\"").unwrap(),
            Endpoint::localhost(8765)
        );

        assert_eq!(
            serde_json::from_str::<Endpoint>("\"localhost:8765\"").unwrap(),
            Endpoint::localhost(8765)
        );

        assert!(serde_json::from_str::<Endpoint>("\"::::\"").is_err());
    }
}
