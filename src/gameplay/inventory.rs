use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::screens::Screen;

pub fn plugin(app: &mut App) {
    app.init_resource::<Inventory>();
    app.add_systems(OnEnter(Screen::Gameplay), spawn_inventory_hud);
    app.add_systems(
        Update,
        update_inventory_hud.run_if(resource_changed::<Inventory>),
    );
    app.add_observer(on_select_slot::<SelectSlot1, 0>);
    app.add_observer(on_select_slot::<SelectSlot2, 1>);
    app.add_observer(on_select_slot::<SelectSlot3, 2>);
}

#[derive(Resource)]
pub(crate) struct Inventory {
    pub slots: [Option<Item>; 3],
    pub active_slot: usize,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            slots: [Some(Item::Shovel), Some(Item::Gun), None],
            active_slot: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Item {
    Shovel,
    Gun,
}

#[derive(Debug, InputAction)]
#[action_output(bool)]
pub(crate) struct SelectSlot1;

#[derive(Debug, InputAction)]
#[action_output(bool)]
pub(crate) struct SelectSlot2;

#[derive(Debug, InputAction)]
#[action_output(bool)]
pub(crate) struct SelectSlot3;

fn on_select_slot<Action: InputAction, const N: usize>(
    _on: On<Start<Action>>,
    mut inventory: ResMut<Inventory>,
) {
    inventory.active_slot = N;
}

const SLOT_SIZE: f32 = 60.0;
const SLOT_GAP: f32 = 8.0;
const ACTIVE_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.4);
const INACTIVE_COLOR: Color = Color::srgba(0.3, 0.3, 0.3, 0.4);

#[derive(Component)]
struct InventorySlotUi(usize);

fn spawn_inventory_hud(mut commands: Commands) {
    commands
        .spawn((
            Name::new("Inventory HUD"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::End,
                padding: UiRect::bottom(Val::Px(20.0)),
                ..default()
            },
            DespawnOnExit(Screen::Gameplay),
        ))
        .with_children(|parent| {
            parent
                .spawn(Node {
                    column_gap: Val::Px(SLOT_GAP),
                    ..default()
                })
                .with_children(|row| {
                    for i in 0..3 {
                        let bg = if i == 0 { ACTIVE_COLOR } else { INACTIVE_COLOR };
                        row.spawn((
                            Name::new(format!("Slot {}", i + 1)),
                            InventorySlotUi(i),
                            Node {
                                width: Val::Px(SLOT_SIZE),
                                height: Val::Px(SLOT_SIZE),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(bg),
                            BorderColor::all(Color::WHITE),
                        ))
                        .with_child((
                            Text::new(""),
                            TextFont {
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    }
                });
        });
}

fn update_inventory_hud(
    inventory: Res<Inventory>,
    mut slots: Query<(&InventorySlotUi, &mut BackgroundColor, &Children)>,
    mut texts: Query<&mut Text>,
) {
    for (slot_ui, mut bg, children) in &mut slots {
        let idx = slot_ui.0;
        *bg = if idx == inventory.active_slot {
            ACTIVE_COLOR
        } else {
            INACTIVE_COLOR
        }
        .into();

        let item_name = inventory.slots[idx]
            .as_ref()
            .map(|item| format!("{:?}", item))
            .unwrap_or(String::new());

        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child) {
                **text = item_name.to_string();
            }
        }
    }
}
