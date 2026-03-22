use bevy::prelude::*;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;

use super::counters::LatencyTracker;
use super::overlay::{
    setup_debug_overlay, toggle_debug_overlay, update_debug_overlay, DebugOverlayVisible,
};

pub struct DiagnosticsPlugin;

impl Plugin for DiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .insert_resource(LatencyTracker::default())
            .insert_resource(DebugOverlayVisible(false))
            .add_systems(Startup, setup_debug_overlay)
            .add_systems(Update, (toggle_debug_overlay, update_debug_overlay));
    }
}
