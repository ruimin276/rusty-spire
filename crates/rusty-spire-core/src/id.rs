use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated, namespaced model identifier such as `CARD.BASH`.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ModelId(String);

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ModelIdError {
    #[error("model id must be NAMESPACE.NAME using uppercase ASCII, digits, or underscores")]
    Invalid,
}

impl ModelId {
    pub fn new(value: impl Into<String>) -> Result<Self, ModelIdError> {
        let value = value.into();
        let Some((namespace, name)) = value.split_once('.') else {
            return Err(ModelIdError::Invalid);
        };
        let valid = |part: &str| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        };
        if !valid(namespace) || !valid(name) || name.contains('.') {
            return Err(ModelIdError::Invalid);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for ModelId {
    type Err = ModelIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_namespaced_model_ids() {
        assert_eq!(
            ModelId::new("CARD.IRON_WAVE").unwrap().as_str(),
            "CARD.IRON_WAVE"
        );
        for invalid in ["", "BASH", "card.BASH", "CARD.", "CARD.Bash", "CARD.A.B"] {
            assert_eq!(ModelId::new(invalid), Err(ModelIdError::Invalid));
        }
    }
}
