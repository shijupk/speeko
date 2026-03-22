use bevy::prelude::*;

use super::events::{RecognizedWordEvent, RejectedWordEvent, RejectionReason};
use super::resources::SpeechReceiver;

pub struct SpeechPlugin;

impl Plugin for SpeechPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<RecognizedWordEvent>()
            .add_event::<RejectedWordEvent>()
            .add_systems(Update, speech_input_system);
    }
}

fn speech_input_system(
    receiver: Option<Res<SpeechReceiver>>,
    mut recognized_events: EventWriter<RecognizedWordEvent>,
    mut rejected_events: EventWriter<RejectedWordEvent>,
) {
    let Some(receiver) = receiver else { return };

    while let Ok(word) = receiver.receiver.try_recv() {
        match word.result.word {
            Some(w) => {
                tracing::info!("Speech accepted: '{}' (confidence={:.2})", w, word.result.confidence);
                recognized_events.send(RecognizedWordEvent {
                    word: w,
                    confidence: word.result.confidence,
                    all_scores: word
                        .result
                        .scores
                        .iter()
                        .map(|s| (s.word.clone(), s.distance))
                        .collect(),
                    recognized_at: word.recognized_at,
                });
            }
            None => {
                tracing::debug!("Speech rejected (confidence={:.2})", word.result.confidence);
                rejected_events.send(RejectedWordEvent {
                    best_guess: word.result.scores.first().map(|s| s.word.clone()),
                    confidence: word.result.confidence,
                    reason: RejectionReason::LowConfidence,
                    recognized_at: word.recognized_at,
                });
            }
        }
    }
}
