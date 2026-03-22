use bevy::prelude::*;

use crate::app_states::AppState;
use crate::config_resource::GameConfigRes;
use crate::speech::resources::SpeechReceiver;

use super::recognizer_thread::{RecognizerThread, RecognizerThreadConfig};
use super::ring_buffer::create_ring_buffer;

/// Holds the recognizer thread handle (Send + Sync).
#[derive(Resource)]
pub struct RecognizerHandle {
    pub _recognizer: RecognizerThread,
}

pub struct AudioBridgePlugin;

impl Plugin for AudioBridgePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_audio_bridge);
    }
}

fn setup_audio_bridge(world: &mut World) {
    let config = world.resource::<GameConfigRes>().0.clone();

    let sample_rate = config.audio.sample_rate;
    let buffer_secs = config.game.ring_buffer.buffer_seconds;

    // Create ring buffer
    let (producer, consumer) = create_ring_buffer(sample_rate, buffer_secs);

    // Start continuous audio capture
    let capture = match vc_audio::continuous_capture::ContinuousCapture::start(
        sample_rate, producer,
    ) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(
                "Failed to start audio capture: {}. Game will run without voice input.",
                e
            );
            return;
        }
    };

    // Create channel for recognizer → ECS communication
    let (sender, receiver) = crossbeam_channel::bounded(16);

    // Build recognizer config
    let rec_config = RecognizerThreadConfig {
        sample_rate,
        dsp: config.dsp.clone(),
        vad: config.vad.clone(),
        mfcc: config.mfcc.clone(),
        model_dir: config.classifier.model_dir.clone(),
        max_frames: config.classifier.max_frames,
        confidence_threshold: config.recognition.confidence_threshold,
        poll_interval_ms: config.game.ring_buffer.recognizer_poll_ms,
        post_recognition_cooldown_ms: config.game.ring_buffer.post_recognition_cooldown_ms,
        use_cmn: config.mfcc.use_cmn,
        use_deltas: config.mfcc.use_deltas,
    };

    // Start recognizer thread
    let recognizer = RecognizerThread::start(consumer, rec_config, sender);

    // Insert resources: capture is NonSend (cpal::Stream is !Send)
    world.insert_non_send_resource(capture);
    world.insert_resource(SpeechReceiver { receiver });
    world.insert_resource(RecognizerHandle {
        _recognizer: recognizer,
    });

    tracing::info!("AudioBridge: capture + recognizer started");
}
