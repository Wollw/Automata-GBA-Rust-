#![no_std]
#![no_main]
// This is required to allow writing tests
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, reexport_test_harness_main = "test_main")]
#![cfg_attr(test, test_runner(agb::test_runner::test_runner))]

extern crate alloc;

const WIDTH  : u16 = 30;
const HEIGHT : u16 = 20;

use::agb::{
    display::{
        tiled::{ RegularBackgroundSize, TiledMap},
        Priority,
    },
    include_background_gfx,
    input::Button,
};

use::alloc::{vec::Vec};

include_background_gfx!(background_tiles, "222222",
    tiles =>  deduplicate "gfx/tiles.aseprite"
);

pub struct Graph {
    nodes: Vec<NodeData>,
    edges: Vec<EdgeData>,
}

pub type NodeIndex = usize;

pub struct NodeData {
    state: u16,
    x: u16,
    y: u16,

    first_outgoing_edge: Option<EdgeIndex>
}

pub type EdgeIndex = usize;

enum Direction {Left, Right, Up, Down}

pub struct EdgeData {
    direction: Option<Button>,
    target: NodeIndex,
    next_outgoing_edge: Option<EdgeIndex>
}

impl Graph {
    
    pub fn new() -> Self {
        Graph { nodes: Vec::new(), edges: Vec::new() }
    }

    pub fn add_node(&mut self, x:u16, y:u16, state:u16) -> NodeIndex {
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
            n += self.nodes[e].state % 2;
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

fn move_cursor(graph : &mut Graph, cursor : NodeIndex, button : Button) -> NodeIndex {
    let mut c = cursor;
    let mut maybe_edge = graph.nodes[c].first_outgoing_edge;
    loop {
        if let Some(edge_index) = maybe_edge {
            let b = graph.edges[edge_index].direction;
            match b {
                Some(b) => {
                    if b == button {
                        c = graph.edges[edge_index].target;
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
    c
}

#[agb::entry]
fn main(mut gba: agb::Gba) -> ! {

    let mut graph = Graph::new();
    for i in 0..WIDTH*HEIGHT {
        graph.add_node(i%WIDTH, i/WIDTH, 0);
    }
    for i in 0..WIDTH {
    for j in 0..HEIGHT {
        let n_right      = (i + 1) % WIDTH + j*WIDTH;
        let n_down       = ((((j+1) % HEIGHT )*WIDTH))+i;
        let n_down_right = ((((j+1) % HEIGHT )*WIDTH))+((i+1)%WIDTH);
        let n_down_left : usize =
              (((j+1) % HEIGHT )*WIDTH) as usize
            + (i as isize -1 as isize).rem_euclid(WIDTH as isize) as usize;
        let n = j*WIDTH+i;
        graph.add_edge(n.into(), (n_right).into(), Some(Button::RIGHT));
        graph.add_edge((n_right).into(), n.into(), Some(Button::LEFT));
        graph.add_edge(n.into(), (n_down).into(), Some(Button::DOWN));
        graph.add_edge((n_down).into(), n.into(), Some(Button::UP));
        graph.add_edge(n.into(), (n_down_right).into(), None);
        graph.add_edge((n_down_right).into(), n.into(), None);
        graph.add_edge(n.into(), (n_down_left).into(), None);
        graph.add_edge((n_down_left).into(), n.into(), None);
    }}
    let mut cursor = (WIDTH+3) as usize;
    //graph.nodes[cursor].state = 3;


    let (gfx, mut vram) = gba.display.video.tiled0();
    let vblank = agb::interrupt::VBlank::get();

    let tileset = background_tiles::tiles.tiles;
    
    vram.set_background_palettes(background_tiles::PALETTES);

    let mut bg = gfx.background(
        Priority::P0,
        RegularBackgroundSize::Background32x32,
        tileset.format(),
    );

    for x in 0..WIDTH {
    for y in 0..HEIGHT {
        bg.set_tile(
            &mut vram,
            (x, y),
            &tileset,
            background_tiles::tiles.tile_settings[0],
        );
    }}

    for n in &graph.nodes {
        bg.set_tile(
            &mut vram,
            (n.x, n.y),
            &tileset,
            background_tiles::tiles.tile_settings[n.state as usize],
        );
    }

    bg.commit(&mut vram);
    bg.set_visible(true);

    let mut input = agb::input::ButtonController::new();

    //Conway's Game of Life
    let rules = [[0,0,0,1,0,0,0,0,0]
                ,[0,0,1,1,0,0,0,0,0]];


    loop {

        vblank.wait_for_vblank();

        input.update();
        let down = input.is_just_pressed(Button::DOWN);
        let up = input.is_just_pressed(Button::UP);
        let left = input.is_just_pressed(Button::LEFT);
        let right = input.is_just_pressed(Button::RIGHT);

        if input.is_pressed(Button::START) {
            // Update State
            let mut neighbors = [0 ; (HEIGHT * WIDTH) as usize];
            for i in 0..graph.nodes.len() {
                neighbors[i] = graph.living_neighbors_count_of(i);
            }

            for i in 0..graph.nodes.len() {
                let next_state = rules[graph.nodes[i].state as usize][neighbors[i] as usize];
                let n = &mut graph.nodes[i];
                n.state = next_state;
                bg.set_tile(
                     &mut vram,
                     (n.x, n.y),
                     &tileset,
                     background_tiles::tiles.tile_settings[n.state as usize],
                     );
            }
        }


        if up {
            cursor = move_cursor(&mut graph, cursor, Button::UP);
        } else if down {
            cursor = move_cursor(&mut graph, cursor, Button::DOWN);
        } else if left {
            cursor = move_cursor(&mut graph, cursor, Button::LEFT);
        } else if right {
            cursor = move_cursor(&mut graph, cursor, Button::RIGHT);
        }


        if input.is_just_pressed(Button::A) {
            let mut n = &mut (graph.nodes)[cursor];
            n.state = (n.state + 1) % 2;
            bg.set_tile(
                 &mut vram,
                 (n.x, n.y),
                 &tileset,
                 background_tiles::tiles.tile_settings[n.state as usize],
                 );
        }

        bg.commit(&mut vram);
    }
}
