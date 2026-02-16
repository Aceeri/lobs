use bevy::prelude::*;
use bevy_trenchbroom::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.add_observer(setup_light);
    app.add_observer(on_flicker_light);
    app.add_systems(Update, animate_flicker);
}

#[point_class(base(Transform, Visibility), size(-4 -4 -4, 4 4 4), color(255 255 0))]
pub(crate) struct Light {
    pub color_r: f32,
    pub color_g: f32,
    pub color_b: f32,
    pub intensity: f32,
    pub range: f32,
    pub radius: f32,
    pub shadows_enabled: bool,
    pub tags: String,
}

impl Default for Light {
    fn default() -> Self {
        Self {
            color_r: 1.0,
            color_g: 1.0,
            color_b: 1.0,
            intensity: 10_000.0,
            range: 20.0,
            radius: 0.05,
            shadows_enabled: true,
            tags: String::new(),
        }
    }
}

/// Parsed tag list from the `tags` property, for matching flicker events.
#[derive(Component)]
struct LightTags(Vec<String>);

impl LightTags {
    fn from_csv(csv: &str) -> Self {
        Self(
            csv.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        )
    }

    fn contains(&self, tag: &str) -> bool {
        self.0.iter().any(|t| t == tag)
    }
}

/// Trigger this event to flicker all lights with a matching tag.
///
/// - `duration`: total time the flicker lasts (seconds)
/// - `frequency`: how many on/off cycles per second
#[derive(Event)]
pub(crate) struct FlickerLight {
    pub tag: String,
    pub duration: f32,
    pub frequency: f32,
}

impl FlickerLight {
    pub fn new(tag: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            duration: 0.4,
            frequency: 10.0,
        }
    }
}

/// Tracks a light mid-flicker, storing the original values to restore.
#[derive(Component)]
struct LightFlicker {
    elapsed: f32,
    duration: f32,
    half_period: f32,
    original_intensity: f32,
}

const FLICKER_DIM_FACTOR: f32 = 0.1;

fn setup_light(add: On<Add, Light>, lights: Query<&Light>, mut commands: Commands) {
    let light = lights.get(add.entity).unwrap();
    let color = Color::linear_rgb(light.color_r, light.color_g, light.color_b);

    commands.entity(add.entity).insert((
        LightTags::from_csv(&light.tags),
        PointLight {
            color,
            intensity: light.intensity,
            radius: light.radius,
            range: light.range,
            shadows_enabled: light.shadows_enabled,
            ..default()
        },
    ));
}

fn on_flicker_light(
    event: On<FlickerLight>,
    mut commands: Commands,
    lights: Query<(Entity, &LightTags, &PointLight), Without<LightFlicker>>,
) {
    let ev = &*event;

    for (entity, tags, point_light) in &lights {
        if !tags.contains(&ev.tag) {
            continue;
        }

        commands.entity(entity).insert(LightFlicker {
            elapsed: 0.0,
            duration: ev.duration,
            half_period: 0.5 / ev.frequency,
            original_intensity: point_light.intensity,
        });
    }
}

fn animate_flicker(
    mut commands: Commands,
    time: Res<Time>,
    mut lights: Query<(Entity, &mut LightFlicker, &mut PointLight)>,
) {
    for (entity, mut flicker, mut point_light) in &mut lights {
        flicker.elapsed += time.delta_secs();

        if flicker.elapsed >= flicker.duration {
            point_light.intensity = flicker.original_intensity;
            commands.entity(entity).remove::<LightFlicker>();
            continue;
        }

        let cycle = (flicker.elapsed / flicker.half_period) as u32;
        let dimmed = cycle % 2 == 0;

        let factor = if dimmed { FLICKER_DIM_FACTOR } else { 1.0 };
        point_light.intensity = flicker.original_intensity * factor;
    }
}
