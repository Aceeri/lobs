use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_trenchbroom::geometry::{Brushes, BrushesAsset};
use bevy_trenchbroom::prelude::*;

use super::player::Player;
use super::tags::Tags;
use crate::third_party::avian3d::CollisionLayer;

/// Returns a system that checks if the player is inside any sensor area
/// matching all of the given tags.
pub(crate) fn player_in_sensor(
    tags: &[&str],
) -> impl FnMut(Query<(&CollidingEntities, &Tags)>, Query<(), With<Player>>) -> bool + Send + Sync
{
    let tags: Vec<String> = tags.iter().map(|s| s.to_string()).collect();
    move |sensors: Query<(&CollidingEntities, &Tags)>, players: Query<(), With<Player>>| {
        sensors.iter().any(|(colliding, sensor_tags)| {
            tags.iter().all(|t| sensor_tags.contains(t))
                && colliding.iter().any(|&e| players.get(e).is_ok())
        })
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, init_sensor_areas);
}

#[solid_class(base(Transform, Visibility))]
pub(crate) struct SensorArea {
    pub tags: String,
}

impl Default for SensorArea {
    fn default() -> Self {
        Self {
            tags: String::new(),
        }
    }
}

#[derive(Component)]
struct SensorAreaReady;

fn init_sensor_areas(
    mut commands: Commands,
    areas: Query<(Entity, &SensorArea, &Brushes), Without<SensorAreaReady>>,
    brushes_assets: Res<Assets<BrushesAsset>>,
) {
    for (entity, area, brushes) in &areas {
        if let Brushes::Shared(handle) = brushes {
            if brushes_assets.get(handle).is_none() {
                continue;
            }
        }

        commands.entity(entity).insert((
            SensorAreaReady,
            Tags::from_csv(&area.tags),
            Sensor,
            CollidingEntities::default(),
            CollisionLayers::new(
                CollisionLayer::Sensor,
                [
                    CollisionLayer::Character,
                    CollisionLayer::Prop,
                    CollisionLayer::Projectile,
                ],
            ),
        ));
    }
}
