//! Versioned, validated static combat data packages.

mod catalog;

pub use catalog::*;

#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("invalid data package JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid data package: {0}")]
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_grouped_package_and_validates_effect_vocabulary() {
        let package = DataPackage::from_json(include_bytes!(
            "../../../packages/spire-codex-stable-v0.107.1.json"
        ))
        .unwrap();
        assert_eq!(package.package_id, "spire-codex-stable-v0.107.1");
        let iron_wave = &package.data.cards["CARD.IRON_WAVE"];
        assert!(matches!(
            iron_wave.effects[0],
            CardEffectDefinition::Block { .. }
        ));
        assert!(matches!(
            iron_wave.effects[1],
            CardEffectDefinition::Damage { .. }
        ));
    }

    #[test]
    fn rejects_unknown_effect_power() {
        let mut value: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../packages/spire-codex-stable-v0.107.1.json"
        ))
        .unwrap();
        value["cards"]["CARD.BASH"]["effects"][1]["id"] = "POWER.UNKNOWN".into();
        let error = DataPackage::from_json(&serde_json::to_vec(&value).unwrap()).unwrap_err();
        assert!(error.to_string().contains("unknown power"));
    }
}
