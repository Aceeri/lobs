//! Reusable UI widgets & theming.

// Unused utilities may trigger this lints undesirably.
#![allow(dead_code)]

pub(crate) mod interaction;
pub(crate) mod palette;
pub(crate) mod widget;

#[allow(unused_imports)]
pub(crate) mod prelude {
    pub(crate) use super::{
        GameFont, TitleFont, interaction::InteractionPalette, palette as ui_palette, widget,
    };
}

use bevy::prelude::*;

/// The game's UI font, used for most text.
#[derive(Resource)]
pub(crate) struct GameFont(pub Handle<Font>);

/// The title font, used only for the big title on the main menu.
#[derive(Resource)]
pub(crate) struct TitleFont(pub Handle<Font>);

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(interaction::plugin);
    let assets = app.world().resource::<AssetServer>();
    let game_font = assets.load("fonts/Fhacondensedfrenchnc-YJ7q.otf");
    let title_font = assets.load("fonts/Goudy Titling W05 Bold.otf");
    app.insert_resource(GameFont(game_font));
    app.insert_resource(TitleFont(title_font));
}
