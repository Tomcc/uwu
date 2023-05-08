use base64_url::base64;
use core::fmt;
use core::fmt::Display;
use serde::Deserialize;
use serde::Serialize;
use std::convert::TryFrom;
use std::convert::TryInto;

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Debug, Hash)]
#[serde(try_from = "String", into = "String")]
pub struct Id64(u64);

impl Id64 {
    pub fn from_rng<RNG>(rng: &mut RNG) -> Self
    where
        RNG: rand::Rng,
    {
        Id64(rng.gen())
    }

    pub fn random() -> Self {
        Self::from_rng(&mut rand::thread_rng())
    }

    pub fn as_bytes(&self) -> &[u8] {
        let ptr: *const u64 = &self.0;
        let ptr = ptr as *const u8;
        unsafe { std::slice::from_raw_parts(ptr, 8) }
    }
}

impl From<u64> for Id64 {
    fn from(id: u64) -> Self {
        Id64(id)
    }
}

impl From<[u8; 8]> for Id64 {
    fn from(bytes: [u8; 8]) -> Self {
        Id64(u64::from_le_bytes(bytes))
    }
}

impl Into<u64> for Id64 {
    fn into(self) -> u64 {
        self.0
    }
}

impl Into<[u8; 8]> for Id64 {
    fn into(self) -> [u8; 8] {
        self.0.to_le_bytes()
    }
}

impl Into<String> for Id64 {
    fn into(self) -> String {
        base64_url::encode(self.as_bytes())
    }
}

// use thiserror to make a TryFrom error struct
#[derive(thiserror::Error, Debug)]
pub enum TryFromStrError {
    #[error("base64 decode error")]
    Base64DecodeError(#[from] base64::DecodeError),
    #[error("Invalid length")]
    TryFromSliceError(#[from] std::array::TryFromSliceError),
}

impl TryFrom<&str> for Id64 {
    type Error = TryFromStrError;
    fn try_from(string: &str) -> Result<Id64, Self::Error> {
        let bytes: Vec<u8> = base64_url::decode(string)?;

        let bytes: [u8; 8] = bytes.as_slice().try_into()?;

        Ok(Id64::from(bytes))
    }
}

impl TryFrom<String> for Id64 {
    type Error = TryFromStrError;
    fn try_from(string: String) -> Result<Id64, Self::Error> {
        TryFrom::try_from(string.as_str())
    }
}

impl Display for Id64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", base64_url::encode(self.as_bytes()))
    }
}
