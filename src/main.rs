#![no_std]
#![no_main]
// This is required to allow writing tests
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, reexport_test_harness_main = "test_main")]
#![cfg_attr(test, test_runner(agb::test_runner::test_runner))]

extern crate alloc;

const WIDTH  : u16 = 30;
const HEIGHT : u16 = 20;
const TILE_SIZE : u16 = 8;

use::agb::{
    display::{
        object::{Object, Graphics, Tag, OamManaged},
        tiled::{ RegularMap, RegularBackgroundSize, TiledMap, VRamManager},
        Priority,
    },
    input::{Tri, Button},
    save::{SaveManager, Error},
    include_background_gfx,
    include_aseprite,
};

use::alloc::{vec::Vec};

use core::ops::Not;

include_background_gfx!(background_tiles, "ff00ff",
    tiles =>  deduplicate "gfx/tiles.aseprite"
);

const SPRITES: &Graphics = include_aseprite!("gfx/sprites.aseprite");
const CURSOR_SPRITE: &Tag = SPRITES.tags().get("Cursor");

pub struct Graph {
    nodes: Vec<NodeData>,
    edges: Vec<EdgeData>,
}

pub type NodeIndex = usize;

#[derive(Debug,PartialEq,Copy,Clone)]
enum CellState {
    Dead, Live
}

impl From<u16> for CellState {
    fn from(item: u16) -> Self {
        match item {
            0 => Dead,
            1 => Live,
            _ => Dead
        }
    }
}

impl Not for CellState {
    type Output = Self;
    fn not(self) -> Self::Output {
        match self {
            Dead => Live,
            Live => Dead
        }
    }
}

use crate::MenuType::*;
use crate::NodeType::*;
use crate::CellState::*;

#[derive(PartialEq,Debug)]
enum MenuType {
    New, Save, Load
}

#[derive(Debug)]
enum NodeType {
    Cell(CellState),
    Menu(MenuType),
}

#[derive(Debug)]
pub struct NodeData {
    state: NodeType,
    x: u16,
    y: u16,
    first_outgoing_edge: Option<EdgeIndex>
}

pub type EdgeIndex = usize;

pub struct EdgeData {
    direction: Option<Button>,
    target: NodeIndex,
    next_outgoing_edge: Option<EdgeIndex>
}

impl Graph {
    
    pub fn new() -> Self {
        Graph { nodes: Vec::new(), edges: Vec::new() }
    }

    pub fn add_node(&mut self, x:u16, y:u16, state:NodeType) -> NodeIndex {
        let index = self.nodes.len();
        self.nodes.push(NodeData { x,y,state,first_outgoing_edge: None });
        index
    }

    pub fn add_edge(&mut self, source: NodeIndex, target: NodeIndex, direction: Option<Button>) {
        let edge_index = self.edges.len();
        let node_data = &mut self.nodes[source];
        self.edges.push(EdgeData {
            direction,
            target: target,
            next_outgoing_edge: node_data.first_outgoing_edge
        });
        node_data.first_outgoing_edge = Some(edge_index);
    }
    
    pub fn successors(&self, source: NodeIndex) -> Successors {
        let first_outgoing_edge = self.nodes[source].first_outgoing_edge;
        Successors { graph: self, current_edge_index: first_outgoing_edge }
    }

    pub fn living_neighbors_count_of(&self, source: NodeIndex) -> u16 {
        let mut n = 0;
        for e in self.successors(source) {
            match self.nodes[e].state {
                Cell(s) => n += s as u16,
                _ => n = n,
            }
        }
        n
    }

}

pub struct Successors<'graph> {
    graph: &'graph Graph,
    current_edge_index: Option<EdgeIndex>
}

impl<'graph> Iterator for Successors<'graph> {
    type Item = NodeIndex;

    fn next(&mut self) -> Option<NodeIndex> {
        match self.current_edge_index {
            None => None,
            Some(edge_num) => {
                let edge = &self.graph.edges[edge_num];
                self.current_edge_index = edge.next_outgoing_edge;
                Some(edge.target)
            }
        }
    }
}

pub struct Cursor<'a> {
    node: NodeIndex,
    x: u16,
    y: u16,
    object: Object<'a>
}

impl<'a> Cursor<'a> {
    pub fn new(graph: &Graph, node: NodeIndex, object: &'a OamManaged) -> Self {
        let mut cursor_object = object.object_sprite(CURSOR_SPRITE.sprite(0));
        cursor_object.hide();
        let mut c = Cursor { node
               , x: graph.nodes[node].x
               , y: graph.nodes[node].y
               , object: cursor_object
               };
        c.redraw(graph);
        c
    }

    fn hide(&mut self) {
        self.object.hide();
    }

    fn show(&mut self) {
        self.object.show();
    }

    fn set_position(&mut self, graph : &Graph, node: NodeIndex) {
        self.node = node;
        self.x = graph.nodes[node].x;
        self.y = graph.nodes[node].y;
        self.redraw(graph);
    }

    fn move_cursor(&mut self, graph : &Graph, button : Button) {
        let mut maybe_edge = graph.nodes[self.node].first_outgoing_edge;
        loop {
            if let Some(edge_index) = maybe_edge {
                let b = graph.edges[edge_index].direction;
                match b {
                    Some(b) => {
                        if b == button {
                            self.node = graph.edges[edge_index].target;
                            break;
                        }
                    },
                    None => ()
                }
                maybe_edge = graph.edges[edge_index].next_outgoing_edge
            } else {
                break;
            }
        }
        self.redraw(graph);
    }

    fn redraw(&mut self, graph : &Graph) {
        self.object.set_x(graph.nodes[self.node].x * TILE_SIZE);
        self.object.set_y(graph.nodes[self.node].y * TILE_SIZE);
    }
}

fn new_world(width: u16, height: u16) -> Graph {
    let mut graph = Graph::new();
    for i in 0..width*height {
        graph.add_node(i%width, i/width, Cell(Dead));
    }
    for i in 0..width {
    for j in 0..height {
        let n_right      = (i + 1) % width + j*width;
        let n_down       = ((((j+1) % height )*width))+i;
        let n_down_right = ((((j+1) % height )*width))+((i+1)%width);
        let n_down_left : usize =
              (((j+1) % height )*width) as usize
            + (i as isize -1 as isize).rem_euclid(width as isize) as usize;
        let n = j*width+i;
        graph.add_edge(n.into(), (n_right).into(), Some(Button::RIGHT));
        graph.add_edge((n_right).into(), n.into(), Some(Button::LEFT));
        graph.add_edge(n.into(), (n_down).into(), Some(Button::DOWN));
        graph.add_edge((n_down).into(), n.into(), Some(Button::UP));
        graph.add_edge(n.into(), (n_down_right).into(), None);
        graph.add_edge((n_down_right).into(), n.into(), None);
        graph.add_edge(n.into(), (n_down_left).into(), None);
        graph.add_edge((n_down_left).into(), n.into(), None);
    }}
    graph
}

fn new_config_menu(bg : &mut RegularMap, vram : &mut VRamManager, settings: &Settings) {
    let tileset = background_tiles::tiles.tiles;

    for x in settings.window_x..settings.window_x+settings.window_width-1 {
    for y in settings.window_y..settings.window_y+settings.window_height-1 {
        bg.set_tile(
            vram,
            (x, y),
            &tileset,
            background_tiles::tiles.tile_settings[1],
        );
    }}

    // Borders
    for x in settings.window_x..settings.window_x+settings.window_width-1 {
        bg.set_tile(
            vram,
            (x, settings.window_y),
            &tileset,
            background_tiles::tiles.tile_settings[4],
        );
        bg.set_tile(
            vram,
            (x, settings.window_y+settings.window_height-1),
            &tileset,
            background_tiles::tiles.tile_settings[4].vflip(true),
        );
    }
    for y in settings.window_y..settings.window_y+settings.window_height-1 {
        bg.set_tile(
            vram,
            (settings.window_x, y),
            &tileset,
            background_tiles::tiles.tile_settings[5],
        );
        bg.set_tile(
            vram,
            (settings.window_x+settings.window_width-1, y),
            &tileset,
            background_tiles::tiles.tile_settings[5].hflip(true),
        );
    }
    bg.set_tile(
        vram,
        (settings.window_x, settings.window_y),
        &tileset,
        background_tiles::tiles.tile_settings[3],
    );
    bg.set_tile(
        vram,
        (settings.window_x+settings.window_width-1, settings.window_y),
        &tileset,
        background_tiles::tiles.tile_settings[3].hflip(true),
    );
    bg.set_tile(
        vram,
        (settings.window_x, settings.window_y+settings.window_height-1),
        &tileset,
        background_tiles::tiles.tile_settings[3].vflip(true),
    );
    bg.set_tile(
        vram,
        (settings.window_x+settings.window_width-1, settings.window_y+settings.window_height-1),
        &tileset,
        background_tiles::tiles.tile_settings[3].hflip(true).vflip(true),
    );

    // Rules
    for x in 0..=8 {
        bg.set_tile(
            vram,
            (settings.window_x+settings.rules_offset_x+x, settings.window_y+settings.rules_offset_y-1),
            &tileset,
            background_tiles::tiles.tile_settings[48+x as usize],
        );
    }
    bg.set_tile(
        vram,
        (settings.window_x+settings.rules_offset_x-1, settings.window_y+settings.rules_offset_y),
        &tileset,
        background_tiles::tiles.tile_settings[8 as usize],
    );
    bg.set_tile(
        vram,
        (settings.window_x+settings.rules_offset_x-1, settings.window_y+settings.rules_offset_y+1),
        &tileset,
        background_tiles::tiles.tile_settings[9 as usize],
    );


    // New/Save/Load Menu
    for x in 0..3 {
        bg.set_tile(
            vram,
            (settings.window_x+settings.rules_offset_x+x, settings.window_y+settings.rules_offset_y+3),
            &tileset,
            background_tiles::tiles.tile_settings[32 + x as usize],
        );
    }
    for x in 0..4 {
        bg.set_tile(
            vram,
            (settings.window_x+settings.rules_offset_x+x, settings.window_y+settings.rules_offset_y+4),
            &tileset,
            background_tiles::tiles.tile_settings[24 + x as usize],
        );
    }
    for x in 0..4 {
        bg.set_tile(
            vram,
            (settings.window_x+settings.rules_offset_x+x, settings.window_y+settings.rules_offset_y+5),
            &tileset,
            background_tiles::tiles.tile_settings[28 + x as usize],
        );
    }
    

}

struct Settings {
    rules: [[u16;9];2],
    speed: u16,
    tiles: [u16;2],

    window_x: u16,
    window_y: u16,
    window_width: u16,
    window_height: u16,
    rules_offset_x: u16,
    rules_offset_y: u16,
}

enum GameState {
    Running,
    Paused,
    Config
}

fn load_world(save: &mut SaveManager, graph: &mut Graph, settings: &mut Settings) -> Result<(),Error> {
    let mut access = save.access()?;

    let mut is_save = 0;
    access.read(0, core::slice::from_mut(&mut is_save))?;

    if is_save != 0 {
        let mut b: u8 = 0;
        let mut i = 0;
        while i < graph.nodes.len() {
            access.read(i, core::slice::from_mut(&mut b))?;
            graph.nodes[i].state = match b {
                b'L' => Cell(Live),
                _ => Cell(Dead)
            };
            i+=1;
        }
        let mut j = 0;
        while j < settings.rules[0].len() {
            access.read(i+j, core::slice::from_mut(&mut b))?;
            settings.rules[0][j] = b.into();
            j+=1;
        }
        let mut k = 0;
        while k < settings.rules[0].len() {
            access.read(i+j+k, core::slice::from_mut(&mut b))?;
            settings.rules[1][k] = b.into();
            k+=1;
        }
    };
    Ok(())

}

fn save_world(save: &mut SaveManager, graph: &Graph, settings: &Settings) -> Result<(), Error> {
    let mut access = save.access()?;

    let mut is_save = 0;
    access.read(0, core::slice::from_mut(&mut is_save))?;

    if is_save != 0 {
        let mut i = 0;
        while i < graph.nodes.len() {
            access.prepare_write(i..i+1)?
                  .write(i, &[
                        match graph.nodes[i].state {
                            Cell(Live) => b'L',
                            Cell(Dead) => b'D',
                            _ => b'X'
                        }]
                  )?;
            i+=1;
        }
        let mut j = 0;
        while j < settings.rules[0].len() {
            access.prepare_write(i+j..i+j+1)?
                  .write(i+j, &[ settings.rules[0][j] as u8 ])?;
            j+=1;
        }
        let mut k = 0;
        while k < settings.rules[1].len() {
            access.prepare_write(i+j+k..i+j+k+1)?
                  .write(i+j+k, &[settings.rules[1][k] as u8 ])?;
            k+=1;
        }
    };
    Ok(())
}

#[agb::entry]
fn main(mut gba: agb::Gba) -> ! {

    gba.save.init_sram();

    // Settings for Conway's Game of Life
    let mut settings = Settings {
            rules: [[0,0,0,1,0,0,0,0,0]
                   ,[0,0,1,1,0,0,0,0,0]],
            speed: 5000,
            tiles: [1,2],

            window_x: WIDTH/4,
            window_y: HEIGHT/4-3,
            window_width: WIDTH/2,
            window_height: HEIGHT/2+1,
            rules_offset_x: 3,
            rules_offset_y: 3,
    };

    let timer = gba.timers.timers();
    let mut timer: agb::timer::Timer = timer.timer2;
    timer.set_divider(agb::timer::Divider::Divider1024);
    timer.set_enabled(false);
    
    // Settings Graph (Rules)
    let mut graph_settings = Graph::new();
    for j in 0..2 {
    for i in 0..9 {
        graph_settings.add_node(
            settings.window_x+settings.rules_offset_x+i,
            settings.window_y+settings.rules_offset_y+j,
            Cell(settings.rules[j as usize][i as usize].into())
        );
    }}
    for j in 0..2 {
    for i in 0..9 {
        if i < 8 {
            graph_settings.add_edge(j*9+i, j*9+i+1, Some(Button::RIGHT));
            graph_settings.add_edge(j*9+i+1, j*9+i, Some(Button::LEFT));
        }
        if j < 1 {
            graph_settings.add_edge(j*9+i, (j+1)*9+i, Some(Button::DOWN));
            graph_settings.add_edge((j+1)*9+i, j*9+i, Some(Button::UP));
        }
    }}

    //Settings Graph (New/Save/Load)
    let node_new = graph_settings.add_node(
            settings.window_x+settings.rules_offset_x,
            settings.window_y+settings.rules_offset_y+3,
            Menu(New));
    let node_save = graph_settings.add_node(
            settings.window_x+settings.rules_offset_x,
            settings.window_y+settings.rules_offset_y+4,
            Menu(Save));
    let node_load = graph_settings.add_node(
            settings.window_x+settings.rules_offset_x,
            settings.window_y+settings.rules_offset_y+5,
            Menu(Load));
    graph_settings.add_edge(node_new, 9, Some(Button::UP));
    graph_settings.add_edge(node_new, node_save, Some(Button::DOWN));
    graph_settings.add_edge(node_save, node_new, Some(Button::UP));
    graph_settings.add_edge(node_save, node_load, Some(Button::DOWN));
    graph_settings.add_edge(node_load, node_save, Some(Button::UP));
    for n in 9..18 {
        graph_settings.add_edge(n, node_new, Some(Button::DOWN));
    }
    
    

    // Game Graph
    let mut graph = new_world(WIDTH.into(), HEIGHT.into());

    let object = gba.display.object.get_managed();
    let mut cursor_world = Cursor::new(&graph, 0, &object);
    let mut cursor_config = Cursor::new(&graph_settings, 16, &object);
    let mut cursor = &mut cursor_world;
    cursor.show();
    object.commit();

    let (gfx, mut vram) = gba.display.video.tiled0();
    let vblank = agb::interrupt::VBlank::get();
    vram.set_background_palettes(background_tiles::PALETTES);


    // Game World Background
    let tileset = background_tiles::tiles.tiles;
    let mut bg = gfx.background(
        Priority::P1,
        RegularBackgroundSize::Background32x32,
        tileset.format(),
    );

    for n in &graph.nodes {
        bg.set_tile(
            &mut vram,
            (n.x, n.y),
            &tileset,
            background_tiles::tiles.tile_settings[
                settings.tiles[
                    match n.state { Cell(s) => s as usize, _ => 0 }
                ] as usize
            ],
        );
    }
    bg.commit(&mut vram);
    bg.set_visible(true);

    //Menu Background
    let mut bg_settings = gfx.background(
        Priority::P0,
        RegularBackgroundSize::Background32x32,
        tileset.format(),
    );
    new_config_menu(&mut bg_settings, &mut vram, &settings);
    bg_settings.commit(&mut vram);
    bg_settings.set_visible(false);


    let mut input = agb::input::ButtonController::new();

    let mut game_state = GameState::Paused;


    timer.set_enabled(true);
    loop {
        input.update();

        match game_state {
            GameState::Paused => {
                if input.is_just_pressed(Button::B) {
                    game_state = GameState::Running;
                    cursor.hide();
                    timer.set_enabled(false);
                    timer.set_enabled(true);
                    continue;
                }

                if input.is_just_pressed(Button::START) {
                    game_state = GameState::Config;
                    bg_settings.set_visible(true);
                    cursor.hide();
                    cursor = &mut cursor_config;
                    cursor.show();
                    cursor.set_position(&mut graph_settings, 18);
                    continue;
                }

                match input.just_pressed_x_tri() {
                    Tri::Negative => cursor.move_cursor(&mut graph, Button::LEFT),
                    Tri::Positive => cursor.move_cursor(&mut graph, Button::RIGHT),
                    _ => ()
                }
                match input.just_pressed_y_tri() {
                    Tri::Negative => cursor.move_cursor(&mut graph, Button::UP),
                    Tri::Positive => cursor.move_cursor(&mut graph, Button::DOWN),
                    _ => ()
                }
                if input.is_just_pressed(Button::A) {
                    let n = &mut (graph.nodes)[cursor.node];
                    match &n.state {
                        Cell(s) => n.state = Cell(!*s),
                        _ => (),
                    }

                    let tile_id = match n.state {
                        Cell(s) => settings.tiles[s as usize],
                        _ => 0,
                    };
                    bg.set_tile(
                         &mut vram,
                         (n.x, n.y),
                         &tileset,
                         background_tiles::tiles.tile_settings[tile_id as usize],
                         );
                }
            },
            GameState::Running => {
                if input.is_just_pressed(Button::B) {
                    game_state = GameState::Paused;
                    cursor.show();
                    continue;
                }

                if timer.value() < settings.speed {
                    vblank.wait_for_vblank();
                    bg.commit(&mut vram);
                    object.commit();
                    continue;
                } else {
                    timer.set_enabled(false);
                    timer.set_enabled(true);
                }

                // Update State
                let mut neighbors = [0 ; (HEIGHT * WIDTH) as usize];
                for i in 0..graph.nodes.len() {
                    neighbors[i] = graph.living_neighbors_count_of(i);
                }

                for i in 0..graph.nodes.len() {
                    let n = &mut graph.nodes[i];
                    match n.state {
                        Cell(s) => {
                            n.state = Cell(settings.rules[s as usize][neighbors[i] as usize].into())
                        },
                        _ => (),
                    }
                    
                    let tile = settings.tiles[
                        match n.state {
                            Cell(s) => s as usize,
                            _ => 0,
                        } as usize];
                    bg.set_tile(
                         &mut vram,
                         (n.x, n.y),
                         &tileset,
                         background_tiles::tiles.tile_settings[tile as usize],
                     );
                }
            },
            GameState::Config => {
                for n in &mut graph_settings.nodes {
                    match n.state {
                        Cell(s) => {

                            let r = &mut settings.rules
                                [(n.y-settings.window_y-settings.rules_offset_y) as usize]
                                [(n.x-settings.window_x-settings.rules_offset_x) as usize];
                            n.state = Cell((*r).into());

                            let tile = settings.tiles[s as usize];
                            bg_settings.set_tile(
                                &mut vram,
                                (n.x, n.y),
                                &tileset,
                                background_tiles::tiles.tile_settings[tile as usize]
                            );
                        }
                        _ => (),
                    }
                }

                if input.is_just_pressed(Button::B) || input.is_just_pressed(Button::START) {
                    game_state = GameState::Paused;
                    bg_settings.set_visible(false);
                    cursor.hide();
                    cursor = &mut cursor_world;
                    cursor.show();
                    timer.set_enabled(false);
                    timer.set_enabled(true);
                    continue;
                }
                match input.just_pressed_x_tri() {
                    Tri::Negative => cursor.move_cursor(&mut graph_settings, Button::LEFT),
                    Tri::Positive => cursor.move_cursor(&mut graph_settings, Button::RIGHT),
                    _ => ()
                }
                match input.just_pressed_y_tri() {
                    Tri::Negative => cursor.move_cursor(&mut graph_settings, Button::UP),
                    Tri::Positive => cursor.move_cursor(&mut graph_settings, Button::DOWN),
                    _ => ()
                }
                if input.is_just_pressed(Button::A) {
                    let mut n = &mut (graph_settings.nodes)[cursor.node];
                    match &n.state {
                        Menu(m) => match m {
                            New => for cell in &mut graph.nodes {
                                cell.state = Cell(Dead);
                                let tile = settings.tiles[match cell.state {
                                    Cell(s) => s as usize,
                                    _ => 0 as usize
                                }] as usize;
                                bg.set_tile(
                                    &mut vram,
                                    (cell.x, cell.y),
                                    &tileset,
                                    background_tiles::tiles.tile_settings[tile]
                                );
                                // Default to Conway's Game of Life rules
                                for i in 0..settings.rules.len() {
                                for j in 0..settings.rules[0].len() {
                                    settings.rules[i][j] = 0;
                                }}
                                settings.rules[0][3] = 1;
                                settings.rules[1][2] = 1;
                                settings.rules[1][3] = 1;
                            },
                            Save => save_world(&mut gba.save, &graph, &settings).expect("REASON"),
                            Load => {
                                load_world(&mut gba.save, &mut graph, &mut settings).expect("REASON");
                                for n in &graph.nodes {
                                    bg.set_tile(
                                        &mut vram,
                                        (n.x, n.y),
                                        &tileset,
                                        background_tiles::tiles.tile_settings[
                                            settings.tiles[
                                                match n.state { Cell(s) => s as usize, _ => 0 }
                                            ] as usize
                                        ],
                                    );
                                };
                                for n in &graph_settings.nodes {
                                    
                                };
                            }
                        },
                        Cell(s) => {
                            n.state = Cell(!*s);
                            let r = &mut settings.rules
                                [(n.y-settings.window_y-settings.rules_offset_y) as usize]
                                [(n.x-settings.window_x-settings.rules_offset_x) as usize];
                            *r = !(*r != 0) as u16;
                        }
                    }
                }
            }
        }

        vblank.wait_for_vblank();
        bg.commit(&mut vram);
        bg_settings.commit(&mut vram);
        object.commit();
    }
}
