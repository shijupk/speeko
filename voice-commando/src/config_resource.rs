use bevy::prelude::*;
use std::ops::Deref;
use vc_common::config::VoiceCommandoConfig;

/// Newtype wrapper so `VoiceCommandoConfig` can be used as a Bevy Resource.
#[derive(Resource)]
pub struct GameConfigRes(pub VoiceCommandoConfig);

impl Deref for GameConfigRes {
    type Target = VoiceCommandoConfig;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<VoiceCommandoConfig> for GameConfigRes {
    fn from(config: VoiceCommandoConfig) -> Self {
        Self(config)
    }
}
