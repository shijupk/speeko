use bevy::prelude::*;
use crossbeam_channel::Receiver;

use crate::audio_bridge::recognizer_thread::RecognizedWord;

/// Bevy resource wrapping the crossbeam receiver for recognized words.
#[derive(Resource)]
pub struct SpeechReceiver {
    pub receiver: Receiver<RecognizedWord>,
}
