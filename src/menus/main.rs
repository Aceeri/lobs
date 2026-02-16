//! The main menu (seen on the title screen).
use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use bevy::ui::Val::*;

use crate::{
    menus::Menu,
    screens::Screen,
    theme::{GameFont, TitleFont, palette::SCREEN_BACKGROUND, widget},
};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Main), spawn_main_menu);
}

fn spawn_main_menu(
    mut commands: Commands,
    mut cursor_options: Single<&mut CursorOptions>,
    font: Res<GameFont>,
    title_font: Res<TitleFont>,
) {
    cursor_options.grab_mode = CursorGrabMode::None;
    let f = &font.0;
    let tf = &title_font.0;
    commands.spawn((
        Name::new("Main Menu"),
        Node {
            position_type: PositionType::Absolute,
            width: Percent(100.0),
            height: Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            justify_content: JustifyContent::FlexStart,
            padding: UiRect::axes(Px(60.0), Px(80.0)),
            row_gap: Px(30.0),
            ..default()
        },
        Pickable::IGNORE,
        BackgroundColor(SCREEN_BACKGROUND),
        GlobalZIndex(2),
        DespawnOnExit(Menu::Main),
        #[cfg(not(target_family = "wasm"))]
        children![
            (
                Text::new("The Lob"),
                widget::text_font(tf, 120.0),
                TextColor(Color::WHITE),
            ),
            widget::button("play", enter_loading_screen, f),
            widget::button("settings", open_settings_menu, f),
            widget::button("credits", open_credits_menu, f),
            widget::button("exit", exit_app, f),
        ],
        #[cfg(target_family = "wasm")]
        children![
            (
                Text::new("The Lob"),
                widget::text_font(tf, 120.0),
                TextColor(Color::WHITE),
            ),
            widget::button("play", enter_loading_screen, f),
            widget::button("settings", open_settings_menu, f),
            widget::button("credits", open_credits_menu, f),
        ],
    ));
}

fn enter_loading_screen(
    _on: On<Pointer<Click>>,
    mut next_screen: ResMut<NextState<Screen>>,
    mut cursor_options: Single<&mut CursorOptions>,
) {
    next_screen.set(Screen::Loading);
    cursor_options.grab_mode = CursorGrabMode::Locked;
}

fn open_settings_menu(_: On<Pointer<Click>>, mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::Settings);
}

fn open_credits_menu(_: On<Pointer<Click>>, mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::Credits);
}

#[cfg(not(target_family = "wasm"))]
fn exit_app(_: On<Pointer<Click>>, mut app_exit: MessageWriter<AppExit>) {
    app_exit.write(AppExit::Success);
}
