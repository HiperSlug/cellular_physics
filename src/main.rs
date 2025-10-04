mod cell;
mod chunk;
mod chunk_map;

use bevy::{prelude::*, window::PrimaryWindow};
use enum_map::{Enum, EnumMap};

use crate::{
    chunk::{Chunk, LEN},
    chunk_map::ChunkMap,
};

const OFFSETS: EnumMap<Dir, IVec2> = EnumMap::from_array([
    ivec2(-1, 0),  // left
    ivec2(1, 0),   // right
    ivec2(-1, -1), // down_left
    ivec2(0, -1),  // down
    ivec2(1, -1),  // down_right
    ivec2(-1, 1),  // up_left
    ivec2(0, 1),   // up
    ivec2(1, 1),   // up_right
]);

#[derive(Enum, Clone, Copy)]
pub enum Dir {
    Left,
    Right,
    DownLeft,
    Down,
    DownRight,
    UpLeft,
    Up,
    UpRight,
}

impl Dir {
    fn inverse(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Down => Self::Up,
            Self::Up => Self::Down,
            Self::DownLeft => Self::UpRight,
            Self::UpRight => Self::DownLeft,
            Self::DownRight => Self::UpLeft,
            Self::UpLeft => Self::DownRight,
        }
    }
}

const SIZE: UVec2 = UVec2::splat(LEN as u32);
const DISPLAY_FACTOR: u32 = 16;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resolution: (SIZE * DISPLAY_FACTOR).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .insert_resource(Time::<Fixed>::from_hz(45.0))
        .init_resource::<CursorCellPos>()
        .init_resource::<Handles>()
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, (step_simulation, mesh_cells).chain())
        .add_systems(Update, (update_cursors_cell_pos, input_set_cells).chain())
        .run();
}

fn setup(mut commands: Commands) {
    let mut map = ChunkMap::default();
    map.insert(ivec2(0, 0), Chunk::EMPTY);
    map.insert(ivec2(1, 0), Chunk::EMPTY);
    map.insert(ivec2(0, 1), Chunk::EMPTY);
    map.insert(ivec2(1, 1), Chunk::EMPTY);
    commands.insert_resource(map);

    commands.spawn(Camera2d);
}

fn step_simulation(mut map: ResMut<ChunkMap>, mut counter: Local<u8>) {
    let n = (*counter + 1) % 3;
    *counter = n;
    map.sub_step(n);
}

fn mesh_cells(
    mut commands: Commands,
    cell_entities: Query<Entity, With<CellMarker>>,
    handles: Res<Handles>,
    map: Res<ChunkMap>,
) {
    for cell_entity in cell_entities {
        commands.entity(cell_entity).despawn();
    }

    for pos in map.iter_some() {
        commands.spawn((
            Transform::from_translation(
                ((pos.as_vec2() + Vec2::splat(0.5)) - (SIZE.as_vec2() / 2.0)).extend(0.0)
                    * DISPLAY_FACTOR as f32,
            ),
            Mesh2d(handles.0.clone()),
            MeshMaterial2d(handles.1.clone()),
            CellMarker,
        ));
    }
}

#[derive(Resource)]
struct Handles(Handle<Mesh>, Handle<ColorMaterial>);

impl FromWorld for Handles {
    fn from_world(world: &mut World) -> Self {
        let mesh = world
            .resource_mut::<Assets<Mesh>>()
            .add(Rectangle::from_length(DISPLAY_FACTOR as f32));
        let color = world
            .resource_mut::<Assets<ColorMaterial>>()
            .add(Color::WHITE);
        Self(mesh, color)
    }
}

#[derive(Component)]
struct CellMarker;

#[derive(Resource, Default)]
struct CursorCellPos(Option<IVec2>);

fn update_cursors_cell_pos(
    mut cursor_cell_pos: ResMut<CursorCellPos>,
    window: Single<&Window, With<PrimaryWindow>>,
    cam_query: Single<(&Camera, &GlobalTransform)>,
) {
    let (camera, camera_transform) = cam_query.into_inner();

    if let Some(cursor_position) = window.cursor_position()
        && let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_position)
    {
        let cell_pos =
            ((world_pos / DISPLAY_FACTOR as f32) + (SIZE.as_vec2() / 2.0) + Vec2::new(0.0, 0.5))
                .as_ivec2();
        if cell_pos.cmplt(SIZE.as_ivec2()).all() && cell_pos.cmpge(IVec2::ZERO).all() {
            cursor_cell_pos.0 = Some(cell_pos);
        } else {
            cursor_cell_pos.0 = None;
        }
    }
}

fn input_set_cells(
    mb_state: Res<ButtonInput<MouseButton>>,
    world_cursor_pos: Res<CursorCellPos>,
    mut map: ResMut<ChunkMap>,
) {
    if let Some(cell_pos) = world_cursor_pos.0 {
        if mb_state.pressed(MouseButton::Left) {
            map.set_dynamic(cell_pos);
        } else if mb_state.pressed(MouseButton::Right) {
            map.set_static(cell_pos);
        } else if mb_state.pressed(MouseButton::Middle) {
            map.set_none(cell_pos);
        }
    }
}
