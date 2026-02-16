use bevy::prelude::*;

mod animation;
pub(crate) mod button;
pub(crate) mod crosshair;
pub(crate) mod crusts;
pub(crate) mod dig;
pub(crate) mod grave;
pub(crate) mod health_ui;
pub(crate) mod inventory;
pub(crate) mod level;
pub(crate) mod npc;
pub(crate) mod objective;
pub(crate) mod player;
pub(crate) mod ragdoll;
pub(crate) mod scenario;
pub(crate) mod sensor_area;
pub(crate) mod store;
pub(crate) mod tags;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((
        animation::plugin,
        button::plugin,
        crosshair::plugin,
        crusts::plugin,
        grave::plugin,
        health_ui::plugin,
        inventory::plugin,
        npc::plugin,
        objective::plugin,
        dig::plugin,
        player::plugin,
        // ragdoll::plugin,
        scenario::plugin,
        sensor_area::plugin,
        store::plugin,
        tags::plugin,
    ));
    // This plugin preloads the level,
    // so make sure to add it last.
    app.add_plugins(level::plugin);
}
