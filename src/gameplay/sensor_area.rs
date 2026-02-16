use avian3d::prelude::*;
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy_trenchbroom::brush::ConvexHull;
use bevy_trenchbroom::geometry::{Brushes, BrushesAsset};
use bevy_trenchbroom::prelude::*;

use super::player::Player;
use super::tags::Tags;

/// Marker storing the half-extents of the sensor's AABB.
#[derive(Component)]
pub(crate) struct SensorBounds(Vec3);

/// Returns a system that checks if the player is inside any sensor area
/// matching all of the given tags. Uses a manual AABB check so the player's
/// collision layers don't need to include Sensor.
pub(crate) fn player_in_sensor(
    tags: &[&str],
) -> impl FnMut(
    Query<(&GlobalTransform, &SensorBounds, &Tags)>,
    Query<&GlobalTransform, With<Player>>,
) -> bool
       + Send
       + Sync {
    let tags: Vec<String> = tags.iter().map(|s| s.to_string()).collect();
    move |sensors: Query<(&GlobalTransform, &SensorBounds, &Tags)>,
          players: Query<&GlobalTransform, With<Player>>| {
        let Ok(player_tf) = players.single() else {
            return false;
        };
        let player_pos = player_tf.translation();
        sensors.iter().any(|(tf, bounds, sensor_tags)| {
            tags.iter().all(|t| sensor_tags.contains(t)) && {
                let center = tf.translation();
                let half = bounds.0;
                (player_pos.x - center.x).abs() <= half.x
                    && (player_pos.y - center.y).abs() <= half.y
                    && (player_pos.z - center.z).abs() <= half.z
            }
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
        let brushes_asset = match brushes {
            Brushes::Owned(asset) => asset,
            Brushes::Shared(handle) => {
                let Some(asset) = brushes_assets.get(handle) else {
                    continue;
                };
                asset
            }
            #[allow(unreachable_patterns)]
            _ => continue,
        };

        let mut min = DVec3::INFINITY;
        let mut max = DVec3::NEG_INFINITY;
        for brush in brushes_asset.iter() {
            if let Some((from, to)) = brush.as_cuboid() {
                min = min.min(from);
                max = max.max(to);
            } else {
                for (vertex, _) in brush.calculate_vertices() {
                    min = min.min(vertex);
                    max = max.max(vertex);
                }
            }
        }

        if !min.is_finite() || !max.is_finite() {
            continue;
        }

        let size = (max - min).as_vec3();
        let center = ((min + max) * 0.5).as_vec3();

        // Strip auto-generated physics from default_solid_scene_hooks.
        commands
            .entity(entity)
            .insert(SensorAreaReady)
            .remove::<(RigidBody, Collider, CollisionLayers)>();

        commands.spawn((
            Tags::from_csv(&area.tags),
            SensorBounds(size / 2.0),
            Transform::from_translation(center),
        ));
    }
}
