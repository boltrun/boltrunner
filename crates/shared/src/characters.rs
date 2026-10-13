use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct PlayerCharacter(pub String);

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct CharacterAnimations {
    pub idle: Option<String>,
    pub walk: Option<String>,
    pub run: Option<String>,
    pub jump: Option<String>,
    pub dash: Option<String>,
    pub death: Option<String>,
    pub walk_speed: Option<f32>,
    pub run_speed: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CharacterDefinition {
    pub id: String,
    pub name: String,
    pub model: String,
    pub author: String,
    pub source: String,
    pub license: String,
    pub scale: f32,
    pub offset: Vec3,
    pub yaw: f32,
    #[serde(default)]
    pub animations: CharacterAnimations,
    #[serde(default)]
    pub rig: Option<HumanoidRig>,
    #[serde(default)]
    pub arms: Option<WeaponArms>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WeaponArms {
    pub right_upper: String,
    pub right_lower: String,
    pub right_hand: String,
    pub left_upper: String,
    pub left_lower: String,
    pub left_hand: String,
    pub height: f32,
    pub forward: f32,
    pub side: f32,
    pub two_handed: bool,
    #[serde(default)]
    pub hanging: Vec<String>,
    #[serde(default)]
    pub right_rotation: Option<Vec3>,
    #[serde(default)]
    pub left_rotation: Option<Vec3>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HumanoidRig {
    pub left_arm: String,
    pub right_arm: String,
    pub left_leg: String,
    pub right_leg: String,
    pub left_knee: String,
    pub right_knee: String,
    pub chest: String,
}

#[derive(Component, Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct CharacterCatalog {
    pub characters: Vec<CharacterDefinition>,
}
impl CharacterCatalog {
    pub fn find(&self, id: &str) -> Option<&CharacterDefinition> {
        self.characters.iter().find(|entry| entry.id == id)
    }
    pub fn accepts(&self, id: &str) -> bool {
        id.is_empty() || self.find(id).is_some()
    }
    pub fn parse(json: &str) -> Result<Self, String> {
        let catalog: Self = serde_json::from_str(json).map_err(|error| error.to_string())?;
        if catalog.characters.len() > 256 {
            return Err("Use at most 256 characters.".into());
        }
        for (index, character) in catalog.characters.iter().enumerate() {
            if let Some(arms) = &character.arms {
                if !arms.height.is_finite()
                    || !(0.25..=0.90).contains(&arms.height)
                    || !arms.forward.is_finite()
                    || !(0.0..=1.0).contains(&arms.forward)
                    || !arms.side.is_finite()
                    || !(-0.5..=0.5).contains(&arms.side)
                    || [
                        &arms.right_upper,
                        &arms.right_lower,
                        &arms.right_hand,
                        &arms.left_upper,
                        &arms.left_lower,
                        &arms.left_hand,
                    ]
                    .into_iter()
                    .any(|name| name.is_empty() || name.len() > 128)
                    || [arms.right_rotation, arms.left_rotation]
                        .into_iter()
                        .flatten()
                        .any(|angle| !angle.is_finite())
                {
                    return Err(format!("Invalid weapon arm rig: {}", character.id));
                }
            }
            if character.id.is_empty()
                || character.id.len() > 64
                || !character.id.bytes().all(|c| {
                    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'-' | b'_')
                })
                || catalog.characters[..index]
                    .iter()
                    .any(|other| other.id == character.id)
            {
                return Err("Character IDs must be unique lowercase slugs.".into());
            }
            if character.name.trim().is_empty()
                || character.name.chars().count() > 80
                || !character.scale.is_finite()
                || !(0.001..=100.0).contains(&character.scale)
                || !character.offset.is_finite()
                || character.offset.abs().max_element() > 100.0
                || !character.yaw.is_finite()
                || [
                    character.animations.walk_speed,
                    character.animations.run_speed,
                ]
                .into_iter()
                .flatten()
                .any(|speed| !speed.is_finite() || !(0.1..=100.0).contains(&speed))
            {
                return Err(format!("Invalid character metadata: {}", character.id));
            }
            let model = &character.model;
            let remote = model.starts_with("https://") || model.starts_with("http://");
            let local =
                model.starts_with("characters/") && !model.split('/').any(|part| part == "..");
            if (!local && !remote)
                || !(model.ends_with(".glb") || model.ends_with(".vrm"))
                || model.contains('#')
                || model.len() > 2048
            {
                return Err("Use a characters/... asset or a direct HTTP(S) GLB/VRM URL.".into());
            }
        }
        Ok(catalog)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChangeCharacter {
    pub character: String,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CharacterReply {
    pub error: Option<String>,
}
