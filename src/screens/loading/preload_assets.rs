//! A loading screen during which game assets are loaded.
//! This reduces stuttering, especially for audio on Wasm.

use bevy::prelude::*;

use super::LoadingScreen;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(LoadingScreen::Assets), skip_to_shaders);
}

fn skip_to_shaders(mut next_screen: ResMut<NextState<LoadingScreen>>) {
    next_screen.set(LoadingScreen::Shaders);
}
