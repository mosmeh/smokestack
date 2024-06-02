pub mod api;
pub mod model;

pub mod serde_uri {
    use http::Uri;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(uri: &Uri, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        uri.to_string().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Uri, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

pub mod serde_uri_option {
    use http::Uri;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(uri: &Option<Uri>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Wrapper<'a>(#[serde(with = "super::serde_uri")] &'a Uri);
        uri.as_ref().map(Wrapper).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Uri>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper(#[serde(with = "super::serde_uri")] Uri);
        let uri = Option::deserialize(deserializer)?;
        Ok(uri.map(|Wrapper(uri)| uri))
    }
}
