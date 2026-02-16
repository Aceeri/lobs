//! A credits menu.

use crate::audio::MusicPool;
use crate::{
    Pause,
    asset_tracking::LoadResource,
    menus::Menu,
    theme::{GameFont, palette::SCREEN_BACKGROUND, prelude::*},
};
use bevy::{
    ecs::spawn::SpawnIter, input::common_conditions::input_just_pressed, prelude::*, ui::Val::*,
};
use bevy_seedling::sample::AudioSample;
use bevy_seedling::sample::SamplePlayer;

const SCROLL_SPEED: f32 = 6.0;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Credits), spawn_credits_menu);
    app.add_systems(
        Update,
        (
            go_back.run_if(in_state(Menu::Credits).and(input_just_pressed(KeyCode::Escape))),
            scroll_credits.run_if(in_state(Menu::Credits)),
        ),
    );

    app.load_resource::<CreditsAssets>();
    app.add_systems(OnEnter(Menu::Credits), start_credits_music);
}

#[derive(Component)]
struct CreditsScroll(f32);

fn spawn_credits_menu(mut commands: Commands, paused: Res<State<Pause>>, font: Res<GameFont>) {
    let f = &font.0;

    // Full-screen root with overflow clipping
    let mut root = commands.spawn((
        Name::new("Credits Screen"),
        DespawnOnExit(Menu::Credits),
        GlobalZIndex(2),
        Node {
            position_type: PositionType::Absolute,
            width: Percent(100.0),
            height: Percent(100.0),
            overflow: Overflow::clip(),
            ..default()
        },
        Pickable::IGNORE,
    ));
    if paused.get() == &Pause(false) {
        root.insert(BackgroundColor(SCREEN_BACKGROUND));
    }

    // Scrolling content column — starts just below the viewport
    root.with_children(|parent| {
        parent.spawn((
            Name::new("Credits Scroll"),
            CreditsScroll(100.0),
            Node {
                position_type: PositionType::Absolute,
                width: Percent(100.0),
                top: Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Px(20.0),
                padding: UiRect::vertical(Px(40.0)),
                ..default()
            },
            children![
                widget::header("created by", f),
                created_by(f),
                widget::header("assets", f),
                assets(f),
            ],
        ));
    });

    // Back button pinned to bottom-left, independent of credits content
    commands.spawn((
        Name::new("Credits Back Button"),
        DespawnOnExit(Menu::Credits),
        GlobalZIndex(3),
        Node {
            position_type: PositionType::Absolute,
            bottom: Px(30.0),
            left: Px(30.0),
            ..default()
        },
        children![widget::button("back", go_back_on_click, f)],
    ));
}

fn scroll_credits(time: Res<Time>, mut query: Query<(&mut CreditsScroll, &mut Node)>) {
    for (mut scroll, mut node) in &mut query {
        scroll.0 -= SCROLL_SPEED * time.delta_secs();
        node.top = Percent(scroll.0);
    }
}

fn created_by(font: &Handle<Font>) -> impl Bundle {
    grid(
        vec![
            ["Joe Shmoe", "Implemented alligator wrestling AI"],
            ["Jane Doe", "Made the music for the alien invasion"],
        ],
        font,
    )
}

fn assets(font: &Handle<Font>) -> impl Bundle {
    grid(
        vec![
            [
                "Bevy logo",
                "All rights reserved by the Bevy Foundation, permission granted for splash screen use when unmodified",
            ],
            ["Button SFX", "CC0 by Jaszunio15"],
            ["Music", "CC BY 3.0 by Kevin MacLeod"],
            ["Ambient music and Footstep SFX", "CC0 by NOX SOUND"],
            [
                "Throw SFX",
                "FilmCow Royalty Free SFX Library License Agreement by Jason Steele",
            ],
            [
                "Fox model",
                "CC0 1.0 Universal by PixelMannen (model), CC BY 4.0 International by tomkranis (Rigging & Animation), CC BY 4.0 International by AsoboStudio and scurest (Conversion to glTF)",
            ],
            [
                "Player model",
                "You can use it commercially without the need to credit me by Drillimpact",
            ],
            ["Vocals", "CC BY 4.0 by Dillon Becker"],
            ["Night Sky HDRI 001", "CC0 by ambientCG"],
            [
                "Rest of the assets",
                "CC BY-NC-SA 3.0 by The Dark Mod Team, converted to Bevy-friendly assets by Jan Hohenheim",
            ],
            [
                "Lobster",
                "(https://skfb.ly/puDOF) by Azazel750 is licensed under Creative Commons Attribution (http://creativecommons.org/licenses/by/4.0/).",
            ],
            [
                "Shovel",
                "(https://skfb.ly/pzFUY) by wasabicats is licensed under Creative Commons Attribution (http://creativecommons.org/licenses/by/4.0/).",
            ],
            [
                "1870s Style Top Hat",
                "(https://skfb.ly/pDTRS) by MadeByYeshe is licensed under Creative Commons Attribution (http://creativecommons.org/licenses/by/4.0/).",
            ],
            [
                "Tommy gun",
                "(https://skfb.ly/o6OHN) by Redpool is licensed under Creative Commons Attribution (http://creativecommons.org/licenses/by/4.0/).",
            ],
            [
                "Crab",
                "(https://skfb.ly/ovttx) by Kaniksu is licensed under Creative Commons Attribution (http://creativecommons.org/licenses/by/4.0/).",
            ],
            [
                "Metal bucket",
                "(https://skfb.ly/6TGrU) by Kozlov Maksim is licensed under Creative Commons Attribution (http://creativecommons.org/licenses/by/4.0/).",
            ],
            [
                "Background music",
                "bryophyta by Mark Lingard source (Free Music Archive https://freemusicarchive.org/music/mark-lingard/fossorial/bryophyta/) is licensed under Creative Commons Attribution (http://creativecommons.org/licenses/by/4.0/).",
            ],
            [
                "Goudy Font",
                "Icons made by https://www.onlinewebfonts.com/icon is licensed by CC BY 4.0",
            ],
        ],
        font,
    )
}

fn grid(content: Vec<[&'static str; 2]>, font: &Handle<Font>) -> impl Bundle {
    let items: Vec<_> = content
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(i, text)| {
            (
                widget::label(text, font),
                Node {
                    justify_self: if i % 2 == 0 {
                        JustifySelf::End
                    } else {
                        JustifySelf::Start
                    },
                    ..default()
                },
            )
        })
        .collect();
    (
        Name::new("Grid"),
        Node {
            display: Display::Grid,
            row_gap: Px(10.0),
            column_gap: Px(30.0),
            grid_template_columns: RepeatedGridTrack::px(2, 400.0),
            ..default()
        },
        Children::spawn(SpawnIter(items.into_iter())),
    )
}

fn go_back_on_click(_: On<Pointer<Click>>, mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::Main);
}

fn go_back(mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::Main);
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
struct CreditsAssets {
    #[dependency]
    music: Handle<AudioSample>,
}

impl FromWorld for CreditsAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            music: assets.load("audio/music/Monkeys Spinning Monkeys.ogg"),
        }
    }
}

fn start_credits_music(mut commands: Commands, credits_music: Res<CreditsAssets>) {
    commands.spawn((
        Name::new("Credits Music"),
        DespawnOnExit(Menu::Credits),
        SamplePlayer::new(credits_music.music.clone()).looping(),
        MusicPool,
    ));
}
