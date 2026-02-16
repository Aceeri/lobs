use std::collections::HashMap;

use bevy::ecs::system::IntoSystem;
use bevy::prelude::*;
use bevy_yarnspinner::prelude::*;

use super::crusts::HudTopLeft;
use super::dig::{VoxelGraves, VoxelSim};
use crate::gameplay::grave::{GraveState, Slotted, SpawnBody};
use crate::gameplay::npc::{Health, SpawnEnemy, SpawnNpc};
use crate::gameplay::tags::Tags;
use crate::props::specific::light::FlickerLight;
use crate::screens::Screen;
use crate::theme::GameFont;
use crate::third_party::bevy_yarnspinner::YarnNode;

pub fn plugin(app: &mut App) {
    app.init_resource::<Objectives>();
    app.add_observer(spawn_objectives_ui);
    app.add_systems(
        Update,
        (
            register_objective_command,
            run_progress_hooks.run_if(in_state(Screen::Gameplay)),
            update_objective_ui.run_if(resource_changed::<Objectives>),
            animate_objective_completion,
        ),
    );
}

#[derive(Resource)]
pub(crate) struct Objectives {
    pub active: String,
    pub objectives: HashMap<String, Objective>,
}

impl Objectives {
    pub fn active(&self) -> Option<&Objective> {
        self.objectives.get(&self.active)
    }

    pub fn active_mut(&mut self) -> Option<&mut Objective> {
        self.objectives.get_mut(&self.active)
    }

    pub fn set_progress(&mut self, sub_id: &str, value: u32) {
        if let Some(obj) = self.active_mut() {
            obj.set_progress(sub_id, value);
        }
    }

    pub fn complete(&mut self, sub_id: &str) {
        if let Some(obj) = self.active_mut() {
            obj.complete(sub_id);
        }
    }
}

impl Default for Objectives {
    fn default() -> Self {
        let mut objectives = HashMap::new();
        objectives.insert(
            "the_molt".to_string(),
            Objective {
                id: "the_molt".to_string(),
                title: "The Molt".to_string(),
                current: 0,
                items: vec![
                    SubObjective::tracked("dig_3", "dig 3 graves", 3)
                        .hook(|voxels: Query<(&VoxelSim, &Tags)>| -> u32 {
                            voxels
                                .iter()
                                .filter(|(sim, tags)| {
                                    tags.contains("tutorial") && sim.air_ratio() >= 0.8
                                })
                                .count() as u32
                        })
                        .on_complete(|mut commands: Commands| {
                            for _ in 0..3 {
                                commands.trigger(SpawnBody::Queue {
                                    spawner_name: "tutorial_spawner".into(),
                                });
                            }
                        })
                        .on_complete(|mut yarn_nodes: Query<(&Tags, &mut YarnNode)>| {
                            for (tags, mut node) in &mut yarn_nodes {
                                if !tags.contains("larry") {
                                    continue;
                                }
                                node.yarn_node = "3_Dug".to_string();
                            }
                        }),
                    SubObjective::tracked("body_3", "put bodies in the graves", 3)
                        .hook(|graves: Query<(&GraveState, &Tags)>| -> u32 {
                            graves
                                .iter()
                                .filter(|(grave, tags)| tags.contains("tutorial") && grave.filled())
                                .count() as u32
                        })
                        .on_complete(|mut yarn_nodes: Query<(&Tags, &mut YarnNode)>| {
                            for (tags, mut node) in &mut yarn_nodes {
                                if !tags.contains("larry") {
                                    continue;
                                }
                                node.yarn_node = "3_Slotted".to_string();
                            }
                        }),
                    SubObjective::tracked("dirt_3", "put dirt in the graves", 3)
                        .hook(
                            |voxels: Query<(&VoxelSim, &Tags, &VoxelGraves)>,
                             graves: Query<&GraveState>|
                             -> u32 {
                                voxels
                                    .iter()
                                    .filter(|(sim, tags, voxel_graves)| {
                                        tags.contains("tutorial")
                                            && sim.air_ratio() <= 0.1
                                            && voxel_graves
                                                .0
                                                .iter()
                                                .any(|&e| graves.get(e).is_ok_and(|g| g.filled()))
                                    })
                                    .count() as u32
                            },
                        )
                        .on_complete(|mut yarn_nodes: Query<(&Tags, &mut YarnNode)>| {
                            for (tags, mut node) in &mut yarn_nodes {
                                if !tags.contains("larry") {
                                    continue;
                                }
                                node.yarn_node = "3_Done".to_string();
                            }
                        }),
                    SubObjective::tracked("store_hit", "shoot the whale in the store", 1)
                        .on_start(|mut commands: Commands| {
                            commands.trigger(FlickerLight::new("tutorial_hallway"));
                            commands.trigger(SpawnNpc::Queue {
                                spawner_name: "tutorial_whale".to_string(),
                            });
                        })
                        .hook(|npcs: Query<(&Tags, &Health)>| -> u32 {
                            let hit = npcs.iter().any(|(tags, health)| {
                                tags.contains("tutorial_whale") && health.0 < 100.0
                            });
                            if hit { 1 } else { 0 }
                        })
                        .on_complete(|mut commands: Commands| {
                            commands.trigger(SpawnEnemy::Queue {
                                spawner_name: "tutorial_octopus".to_string(),
                            });
                        }),
                    SubObjective::tracked("bury_whale", "bury the whale", 1).hook(
                        |bodies: Query<&Tags, With<Slotted>>| -> u32 {
                            let buried = bodies.iter().any(|tags| tags.contains("tutorial_whale"));
                            if buried { 1 } else { 0 }
                        },
                    ),
                    SubObjective::tracked("help_larry", "help larry!!!", 1),
                    SubObjective::tracked(
                        "bury_whale_octopi",
                        "bury the whale... and the octopi",
                        3,
                    )
                    .on_complete(|mut commands: Commands| {
                        // complete `the_molt` and
                        // swap to `the_job` objective.
                    }),
                ],
            },
        );

        objectives.insert(
            "the_job".to_string(),
            Objective {
                id: "the_job".to_string(),
                title: "The Job".to_string(),
                current: 0,
                items: vec![],
            },
        );

        Self {
            active: "the_molt".to_string(),
            objectives,
        }
    }
}

pub(crate) struct Objective {
    pub id: String,
    pub title: String,
    pub current: usize,
    pub items: Vec<SubObjective>,
}

impl Objective {
    pub fn set_progress(&mut self, sub_id: &str, value: u32) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == sub_id) {
            if let ObjectiveTarget::Tracked { current, target } = &mut item.target {
                *current = value;
                if *current >= *target {
                    item.completed = true;
                }
            }
        }
    }

    pub fn complete(&mut self, sub_id: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == sub_id) {
            item.completed = true;
            if let ObjectiveTarget::Binary { done } = &mut item.target {
                *done = true;
            }
        }
    }
}

type ProgressHookFn = Box<dyn FnMut(&mut ObjectiveTarget, &mut World) + Send + Sync>;
type LifecycleHookFn = Box<dyn FnMut(&mut World) + Send + Sync>;

pub(crate) struct SubObjective {
    pub id: String,
    pub label: String,
    pub target: ObjectiveTarget,
    pub completed: bool,
    started: bool,
    progress_hooks: Vec<ProgressHookFn>,
    on_start_hooks: Vec<LifecycleHookFn>,
    on_complete_hooks: Vec<LifecycleHookFn>,
}

impl SubObjective {
    fn binary(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            target: ObjectiveTarget::Binary { done: false },
            completed: false,
            started: false,
            progress_hooks: Vec::new(),
            on_start_hooks: Vec::new(),
            on_complete_hooks: Vec::new(),
        }
    }

    fn tracked(id: &str, label: &str, target: u32) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            target: ObjectiveTarget::Tracked { current: 0, target },
            completed: false,
            started: false,
            progress_hooks: Vec::new(),
            on_start_hooks: Vec::new(),
            on_complete_hooks: Vec::new(),
        }
    }

    pub fn hook<M, Out>(
        mut self,
        system: impl IntoSystem<(), Out, M> + Send + Sync + 'static,
    ) -> Self
    where
        Out: ProgressUpdate + 'static,
        M: 'static,
    {
        let mut system = IntoSystem::into_system(system);
        let mut initialized = false;
        self.progress_hooks.push(Box::new(move |target, world| {
            if !initialized {
                system.initialize(world);
                initialized = true;
            }
            if let Ok(result) = system.run((), world) {
                result.apply(target);
            }
            system.apply_deferred(world);
        }));
        self
    }

    pub fn on_start<M>(mut self, system: impl IntoSystem<(), (), M> + Send + Sync + 'static) -> Self
    where
        M: 'static,
    {
        let mut system = IntoSystem::into_system(system);
        let mut initialized = false;
        self.on_start_hooks.push(Box::new(move |world| {
            if !initialized {
                system.initialize(world);
                initialized = true;
            }
            let _ = system.run((), world);
            system.apply_deferred(world);
        }));
        self
    }

    pub fn on_complete<M>(
        mut self,
        system: impl IntoSystem<(), (), M> + Send + Sync + 'static,
    ) -> Self
    where
        M: 'static,
    {
        let mut system = IntoSystem::into_system(system);
        let mut initialized = false;
        self.on_complete_hooks.push(Box::new(move |world| {
            if !initialized {
                system.initialize(world);
                initialized = true;
            }
            let _ = system.run((), world);
            system.apply_deferred(world);
        }));
        self
    }
}

pub(crate) enum ObjectiveTarget {
    Binary { done: bool },
    Tracked { current: u32, target: u32 },
}

impl ObjectiveTarget {
    pub fn is_complete(&self) -> bool {
        match self {
            ObjectiveTarget::Binary { done } => *done,
            ObjectiveTarget::Tracked { current, target } => *current >= *target,
        }
    }

    fn debug_value(&self) -> String {
        match self {
            ObjectiveTarget::Binary { done } => format!("{done}"),
            ObjectiveTarget::Tracked { current, target } => format!("{current}/{target}"),
        }
    }
}

pub(crate) trait ProgressUpdate {
    fn apply(self, target: &mut ObjectiveTarget);
}

impl ProgressUpdate for u32 {
    fn apply(self, target: &mut ObjectiveTarget) {
        if let ObjectiveTarget::Tracked { current, .. } = target {
            *current = self;
        }
    }
}

impl ProgressUpdate for bool {
    fn apply(self, target: &mut ObjectiveTarget) {
        if let ObjectiveTarget::Binary { done } = target {
            *done = self;
        }
    }
}

fn run_progress_hooks(world: &mut World) {
    let Some(mut objectives) = world.remove_resource::<Objectives>() else {
        warn!("Objectives resource missing, skipping hooks");
        return;
    };

    let Some(active) = objectives.active_mut() else {
        world.insert_resource(objectives);
        return;
    };

    let current = active.current;
    let Some(item) = active.items.get_mut(current) else {
        world.insert_resource(objectives);
        return;
    };

    if !item.started {
        item.started = true;
        info!("Objective '{}' started", item.id);
        for hook in &mut item.on_start_hooks {
            hook(world);
        }
    }

    if !item.completed && !item.progress_hooks.is_empty() {
        let before = item.target.debug_value();
        for hook in &mut item.progress_hooks {
            hook(&mut item.target, world);
        }
        let after = item.target.debug_value();
        if before != after {
            info!("Objective '{}': {} -> {}", item.id, before, after);
        }
        item.completed = item.target.is_complete();
    }

    if item.completed {
        info!("Objective '{}' completed!", item.id);
        for hook in &mut item.on_complete_hooks {
            hook(world);
        }
        active.current += 1;

        if let Some(next) = active.items.get_mut(active.current) {
            if !next.started {
                next.started = true;
                info!("Objective '{}' started", next.id);
                for hook in &mut next.on_start_hooks {
                    hook(world);
                }
            }
        }
    }

    world.insert_resource(objectives);
}

fn register_objective_command(
    mut runners: Query<&mut DialogueRunner, Added<DialogueRunner>>,
    mut commands: Commands,
) {
    for mut runner in &mut runners {
        let system = commands.register_system(
            |In((id, completed)): In<(String, bool)>, mut objectives: ResMut<Objectives>| {
                if completed {
                    objectives.complete(&id);
                }
            },
        );
        runner.commands_mut().add_command("objective", system);
    }
}

#[derive(Component)]
struct ObjectiveRow(usize);

#[derive(Component)]
struct ObjectiveText(usize);

#[derive(Component)]
struct ObjectiveProgress(usize);

#[derive(Component)]
struct ObjectiveStrike(usize);

#[derive(Component)]
struct ObjectivePanel;

#[derive(Component)]
struct WasCompleted(bool);

#[derive(Component)]
struct ObjectiveCompleteAnim(Timer);

const COMPLETE_ANIM_DURATION: f32 = 0.6;
const COMPLETED_COLOR: Color = Color::srgba(0.6, 0.6, 0.6, 1.0);

fn spawn_objectives_ui(
    add: On<Add, HudTopLeft>,
    mut commands: Commands,
    objectives: Res<Objectives>,
    font: Res<GameFont>,
) {
    let hud_root = add.entity;

    let Some(active) = objectives.active() else {
        return;
    };

    let panel = commands
        .spawn((
            ObjectivePanel,
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .with_children(|panel| {
            // Title
            panel.spawn((
                Text::new(&active.title),
                TextFont {
                    font: font.0.clone(),
                    font_size: 28.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            // Divider
            panel.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(1.0),
                    margin: UiRect::vertical(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::WHITE),
            ));

            // Sub-objectives: show completed + current, hide future
            let current = active.current;
            for (i, item) in active.items.iter().enumerate() {
                let is_completed = item.completed;
                let is_current = i == current;
                let row_visible = if is_completed || is_current {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };

                let progress = match &item.target {
                    ObjectiveTarget::Tracked { current, target } => {
                        format!("{}/{}", current, target)
                    }
                    ObjectiveTarget::Binary { .. } => String::new(),
                };

                panel
                    .spawn((
                        ObjectiveRow(i),
                        WasCompleted(is_completed),
                        Node {
                            position_type: PositionType::Relative,
                            width: Val::Percent(100.0),
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        },
                        row_visible,
                    ))
                    .with_children(|row| {
                        let text_color = if is_completed {
                            Color::srgba(0.6, 0.6, 0.6, 1.0)
                        } else {
                            Color::WHITE
                        };
                        row.spawn((
                            ObjectiveText(i),
                            Text::new(&item.label),
                            TextFont {
                                font: font.0.clone(),
                                font_size: 20.0,
                                ..default()
                            },
                            TextColor(text_color),
                        ));

                        if !progress.is_empty() {
                            row.spawn((
                                ObjectiveProgress(i),
                                Text::new(progress),
                                TextFont {
                                    font: font.0.clone(),
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(text_color),
                            ));
                        }

                        let (strike_visible, strike_width) = if is_completed {
                            (Visibility::Inherited, Val::Percent(100.0))
                        } else {
                            (Visibility::Hidden, Val::Percent(0.0))
                        };
                        row.spawn((
                            ObjectiveStrike(i),
                            Node {
                                position_type: PositionType::Absolute,
                                height: Val::Px(1.0),
                                width: strike_width,
                                top: Val::Percent(50.0),
                                left: Val::Px(0.0),
                                ..default()
                            },
                            BackgroundColor(COMPLETED_COLOR),
                            strike_visible,
                        ));
                    });
            }
        })
        .id();

    commands.entity(hud_root).add_child(panel);
}

fn update_objective_ui(
    mut commands: Commands,
    objectives: Res<Objectives>,
    mut row_query: Query<(Entity, &ObjectiveRow, &mut Visibility, &mut WasCompleted)>,
    mut text_query: Query<(&ObjectiveText, &mut Text, &mut TextColor), Without<ObjectiveProgress>>,
    mut progress_query: Query<
        (&ObjectiveProgress, &mut Text, &mut TextColor),
        Without<ObjectiveText>,
    >,
    mut strike_query: Query<(&ObjectiveStrike, &mut Visibility, &mut Node), Without<ObjectiveRow>>,
) {
    let Some(active) = objectives.active() else {
        return;
    };

    let current = active.current;

    // Detect newly completed rows and start animations
    for (entity, row, mut vis, mut was_completed) in &mut row_query {
        let i = row.0;
        let Some(item) = active.items.get(i) else {
            continue;
        };

        *vis = if i <= current {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };

        // Transition: not completed → completed — start animation
        if item.completed && !was_completed.0 {
            was_completed.0 = true;
            commands
                .entity(entity)
                .insert(ObjectiveCompleteAnim(Timer::from_seconds(
                    COMPLETE_ANIM_DURATION,
                    TimerMode::Once,
                )));
        }
    }

    // Update label text
    for (obj_text, mut text, mut color) in &mut text_query {
        let Some(item) = active.items.get(obj_text.0) else {
            continue;
        };
        **text = item.label.clone();
        if !item.completed {
            *color = TextColor(Color::WHITE);
        }
    }

    // Update progress text
    for (obj_progress, mut text, mut color) in &mut progress_query {
        let Some(item) = active.items.get(obj_progress.0) else {
            continue;
        };
        **text = match &item.target {
            ObjectiveTarget::Tracked { current, target } => format!("{}/{}", current, target),
            ObjectiveTarget::Binary { .. } => String::new(),
        };
        if !item.completed {
            *color = TextColor(Color::WHITE);
        }
    }

    // Make strikethrough visible when completed, but start at 0% width for newly animated ones
    for (obj_strike, mut visibility, mut node) in &mut strike_query {
        let Some(item) = active.items.get(obj_strike.0) else {
            continue;
        };
        if item.completed {
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

fn animate_objective_completion(
    mut commands: Commands,
    time: Res<Time>,
    mut rows: Query<(Entity, &ObjectiveRow, &Children, &mut ObjectiveCompleteAnim)>,
    mut texts: Query<&mut TextColor, With<ObjectiveText>>,
    mut progress_texts: Query<&mut TextColor, (With<ObjectiveProgress>, Without<ObjectiveText>)>,
    mut strikes: Query<(&mut Node, &mut BackgroundColor), With<ObjectiveStrike>>,
) {
    for (entity, _row, children, mut anim) in &mut rows {
        anim.0.tick(time.delta());
        let t = anim.0.fraction();
        // Ease-out for a quick slash feel
        let eased = 1.0 - (1.0 - t) * (1.0 - t);

        for child in children.iter() {
            let lerped_color = {
                let r = 1.0 - (1.0 - 0.6) * eased;
                Color::srgba(r, r, r, 1.0)
            };
            if let Ok(mut color) = texts.get_mut(child) {
                color.0 = lerped_color;
            }
            if let Ok(mut color) = progress_texts.get_mut(child) {
                color.0 = lerped_color;
            }
            if let Ok((mut node, mut bg)) = strikes.get_mut(child) {
                node.width = Val::Percent(eased * 100.0);
                bg.0 = Color::srgba(0.6, 0.6, 0.6, eased);
            }
        }

        if anim.0.just_finished() {
            commands.entity(entity).remove::<ObjectiveCompleteAnim>();
        }
    }
}
