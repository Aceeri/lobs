use bevy::prelude::*;
use bevy_hanabi::prelude::{Gradient as HanabiGradient, *};

use crate::gameplay::{
    dig::VOXEL_SIZE,
    inventory::{AnimationState, DIG_RADIUS},
};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<DigParticleEffect>();
    app.init_resource::<MuzzleFlashEffect>();

    app.add_systems(Update, update_particle_effect_state);
    app.add_observer(start_effect_disabled);
}

#[derive(Component, Reflect)]
#[relationship(relationship_target = ParticleEffects)]
pub struct ParticleEffectOf(pub Entity);

#[derive(Component, Reflect)]
#[relationship_target(relationship = ParticleEffectOf)]
pub struct ParticleEffects(Entity);

impl ParticleEffects {
    pub fn entity(&self) -> Entity {
        self.0
    }
}

fn update_particle_effect_state(
    input: Res<ButtonInput<MouseButton>>,
    children: Query<(&AnimationState, &ParticleEffects), Changed<AnimationState>>,
    mut effects: Query<&mut EffectSpawner, With<ParticleEffectOf>>,
) {
    for (animation_state, child) in children {
        let Ok(mut effect) = effects.get_mut(child.0) else {
            continue;
        };
        match *animation_state {
            AnimationState::Swinging => {
                effect.active = true;
            }
            AnimationState::Resting => {
                effect.active = false;
            }
            AnimationState::Returning => {}
        }
    }
}

fn start_effect_disabled(
    trigger: On<Add, EffectSpawner>,
    mut effects: Query<&mut EffectSpawner, With<ParticleEffectOf>>,
) {
    let Ok(mut effect_spawner) = effects.get_mut(trigger.entity) else {
        return;
    };
    effect_spawner.active = false;
}

#[derive(Resource)]
pub struct DigParticleEffect(pub Handle<EffectAsset>);

impl FromWorld for DigParticleEffect {
    fn from_world(world: &mut World) -> Self {
        let mut effects = world.resource_mut::<Assets<EffectAsset>>();

        let writer = ExprWriter::new();

        let init_vel = SetAttributeModifier::new(
            Attribute::VELOCITY,
            writer
                .lit(Vec3::new(0.0, 2.0, 0.0))
                .uniform(writer.lit(Vec3::new(0.0, 3.0, 0.0)))
                .expr(),
        );

        let mut module = writer.finish();

        let init_pos = SetPositionSphereModifier {
            center: module.lit(Vec3::ZERO),
            radius: module.lit(DIG_RADIUS * VOXEL_SIZE),
            dimension: ShapeDimension::Volume,
        };

        let lifetime = SetAttributeModifier::new(Attribute::LIFETIME, module.lit(0.4));

        let accel = AccelModifier::new(module.lit(Vec3::new(0.0, -9.8, 0.0)));

        let mut gradient = HanabiGradient::new();
        gradient.add_key(0.0, Vec4::new(0.55, 0.35, 0.15, 1.0));
        gradient.add_key(0.7, Vec4::new(0.4, 0.25, 0.1, 0.8));
        gradient.add_key(1.0, Vec4::new(0.3, 0.2, 0.05, 0.0));

        let mut size_curve = HanabiGradient::new();
        size_curve.add_key(0.0, Vec3::splat(0.08));
        size_curve.add_key(1.0, Vec3::splat(0.02));

        let effect = EffectAsset::new(256, SpawnerSettings::once(20.0.into()), module)
            .with_name("DigDirt")
            .init(init_pos)
            .init(init_vel)
            .init(lifetime)
            .update(accel)
            .render(ColorOverLifetimeModifier {
                gradient,
                ..default()
            })
            .render(SizeOverLifetimeModifier {
                gradient: size_curve,
                screen_space_size: false,
            })
            .render(OrientModifier {
                rotation: None,
                mode: OrientMode::FaceCameraPosition,
            });

        Self(effects.add(effect))
    }
}

#[derive(Resource)]
pub struct MuzzleFlashEffect(pub Handle<EffectAsset>);

impl FromWorld for MuzzleFlashEffect {
    fn from_world(world: &mut World) -> Self {
        let mut effects = world.resource_mut::<Assets<EffectAsset>>();

        let writer = ExprWriter::new();

        let mean_vel = writer.lit(Vec3::new(0.0, 0.0, -8.0));
        let sd_vel = writer.lit(Vec3::new(3.0, 3.0, 4.0));
        let init_vel =
            SetAttributeModifier::new(Attribute::VELOCITY, mean_vel.normal(sd_vel).expr());

        let mut module = writer.finish();

        let init_pos = SetPositionSphereModifier {
            center: module.lit(Vec3::ZERO),
            radius: module.lit(0.05),
            dimension: ShapeDimension::Volume,
        };

        let lifetime = SetAttributeModifier::new(Attribute::LIFETIME, module.lit(0.15));

        let mut gradient = HanabiGradient::new();
        gradient.add_key(0.0, Vec4::new(1.0, 0.9, 0.3, 1.0));
        gradient.add_key(0.4, Vec4::new(1.0, 0.6, 0.1, 0.8));
        gradient.add_key(1.0, Vec4::new(0.8, 0.3, 0.0, 0.0));

        let mut size_curve = HanabiGradient::new();
        size_curve.add_key(0.0, Vec3::splat(0.06));
        size_curve.add_key(1.0, Vec3::splat(0.01));

        let effect = EffectAsset::new(128, SpawnerSettings::once(10.0.into()), module)
            .with_name("MuzzleFlash")
            .with_alpha_mode(bevy_hanabi::AlphaMode::Add)
            .init(init_pos)
            .init(init_vel)
            .init(lifetime)
            .render(ColorOverLifetimeModifier {
                gradient,
                ..default()
            })
            .render(SizeOverLifetimeModifier {
                gradient: size_curve,
                screen_space_size: false,
            });

        // Self(effects.add(effect))
        Self(effects.add(fire_effect()))
    }
}

pub fn fire_effect() -> EffectAsset {
    let mut color_gradient1 = HanabiGradient::new();
    color_gradient1.add_key(0.0, Vec4::splat(1.0));
    color_gradient1.add_key(0.1, Vec4::new(1.0, 1.0, 0.0, 1.0));
    color_gradient1.add_key(0.4, Vec4::new(1.0, 0.0, 0.0, 1.0));
    color_gradient1.add_key(1.0, Vec4::splat(0.0));
    let mut size_gradient1 = HanabiGradient::new();
    size_gradient1.add_key(0.0, Vec3::splat(0.06));
    size_gradient1.add_key(0.5, Vec3::splat(0.04));
    size_gradient1.add_key(1.0, Vec3::splat(0.01));

    let writer = ExprWriter::new();

    let init_age = SetAttributeModifier::new(Attribute::AGE, writer.lit(0.).expr());

    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, writer.lit(5.).expr());

    let mean_vel = writer.lit(Vec3::new(0.0, 0.0, -8.0));
    let sd_vel = writer.lit(Vec3::new(3.0, 3.0, -4.0));

    let init_vel = SetAttributeModifier::new(Attribute::VELOCITY, mean_vel.normal(sd_vel).expr());

    // Add constant downward acceleration to simulate gravity
    let accel1 = writer.lit(-Vec3::Y * 3.).expr();
    let update_accel1 = AccelModifier::new(accel1);

    let init_pos1 = SetPositionCone3dModifier {
        base_radius: writer.lit(1.).expr(),
        top_radius: writer.lit(1.).expr(),
        height: writer.lit(0.5).expr(),
        dimension: ShapeDimension::Surface,
    };

    EffectAsset::new(32768, SpawnerSettings::rate(1000.0.into()), writer.finish())
        .with_name("emit:rate")
        .init(init_pos1)
        .init(init_vel)
        .init(init_age)
        .init(init_lifetime)
        .update(update_accel1)
        .render(ColorOverLifetimeModifier::new(color_gradient1))
        .render(SizeOverLifetimeModifier {
            gradient: size_gradient1,
            screen_space_size: false,
        })
        .render(OrientModifier::new(OrientMode::FaceCameraPosition))
}
