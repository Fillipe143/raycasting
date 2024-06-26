use std::{ops::{Add, AddAssign, Div, Mul, MulAssign, Sub, SubAssign}, process::exit, usize};

use raylib::{color::Color, drawing::{RaylibDraw, RaylibDrawHandle}, math::{Rectangle, Vector2}, texture::Texture2D, RaylibHandle};
use raylib::RaylibThread;

const WINDOW_SIZE: Vector2 = Vector2::new(860.0, 860.0);
const MINIMAP_ASPECT_RATIO: f32 = 0.2; // 20%

const EPS: f32 = 1e-6;
const FOV: f32 = 90.0;
const NUM_OF_RAYS: usize = 430;
const FAR_CLIPING_PLANE: f32 = 10.0;

enum Cell {
    EMPTY,
    COLOR(Color),
    TEXTURE(Texture2D),
    TranslucentTexture(Texture2D)
}

struct Board<'a> {
    rows: usize,
    cols: usize,
    cells: Vec<&'a Cell>
}

struct Player {
    pos: Vector2,
    dir: Vector2,
    spd: Vector2,
    turn_spd: f32
}

struct Game<'a> {
    board: Board<'a>,
    player: Player
}

struct Transform2D {
    offset: Vector2,
    zoom: Vector2
}

struct Straight {
    a: f32,
    b: f32,
    dir: Vector2
}

impl<'a> Board<'a> {
    fn new(rows: usize, cols: usize) -> Board<'a> {
        Board {
            rows, cols,
            cells: vec![&Cell::EMPTY; rows * cols]
        }
    }

    fn at(&self, x: usize, y: usize) -> &Cell {
        assert!(x < self.cols, "X out of bounds");
        assert!(y < self.rows, "Y out of bounds");
        self.cells[y * self.cols + x]
    }

    fn set(&mut self, x: usize, y: usize, cell: &'a Cell) {
        assert!(x < self.cols, "X out of bounds");
        assert!(y < self.rows, "Y out of bounds");
        self.cells[y * self.cols + x] = cell
    }
}

impl Player {
    fn new(x: f32, y: f32) -> Player {
        Player {
            pos: Vector2::new(x, y),
            dir: Vector2::new(1.0, 0.0),
            spd: Vector2::one(),
            turn_spd: std::f32::consts::FRAC_PI_2
        }
    }

    fn move_forward(&mut self, delta: f32) {
        self.pos.add_assign(self.spd.mul(delta).mul(self.dir))
    }

    fn move_backward(&mut self, delta: f32) {
        self.pos.sub_assign(self.spd.mul(delta).mul(self.dir))
    }

    fn turn_left(&mut self, delta: f32) {
        self.dir.rotate(-self.turn_spd * delta)
    }

    fn turn_right(&mut self, delta: f32) {
        self.dir.rotate(self.turn_spd * delta)
    }
}

impl Transform2D {
    fn default() -> Transform2D {
        Transform2D {
            offset: Vector2::zero(),
            zoom: Vector2::one()
        }
    }
}

trait Transform2DApplayer {
    fn apply(&self, t: &Transform2D) -> Self;
    fn apply_zoom(&self, t: &Transform2D) -> Self;
}

impl Transform2DApplayer for Vector2 {
    fn apply(&self, t: &Transform2D) -> Vector2 {
        self.mul(t.zoom).add(t.offset)
    }

    fn apply_zoom(&self, t: &Transform2D) -> Vector2 {
        self.mul(t.zoom)
    }
}

impl Transform2DApplayer for Rectangle {
    fn apply(&self, t: &Transform2D) -> Rectangle {
        Rectangle::new(
            (self.x * t.zoom.x) + t.offset.x,
            (self.y * t.zoom.y) + t.offset.y,
            self.width * t.zoom.x,
            self.height * t.zoom.y
        )
    }

    fn apply_zoom(&self, t: &Transform2D) -> Self {
        Rectangle::new(
            self.x * t.zoom.x,
            self.y * t.zoom.y,
            self.width * t.zoom.x,
            self.height * t.zoom.y
        )
    }
}

impl Straight {
    fn new(p1: Vector2, p2: Vector2) -> Straight {
        let dir = p2.sub(p1);

        let a = if dir.x != 0.0 { dir.y / dir.x }
        else { 0.0 };

        let b = p1.y - (p1.x * a);

        Straight { a, b, dir }
    }

    fn f(&self, x: f32) -> f32 {
        (x * self.a) + self.b
    }

    fn f1(&self, y: f32) -> f32 {
        (y - self.b) / self.a
    }
}

fn next_ray_step(current: Vector2, straight: &Straight) -> Vector2 {
    let x = if straight.dir.x > 0.0 { f32::ceil(current.x) }
    else { f32::floor(current.x) };
    let y = straight.f(x);

    if straight.a != 0.0 {
        let y2 = if straight.dir.y > 0.0 { f32::ceil(current.y) }
        else { f32::floor(current.y) };
        let x2 = straight.f1(y2);

        if Vector2::new(x2, y2).sub(current).length_sqr() < Vector2::new(x, y).sub(current).length_sqr() {
            return Vector2::new(x2, y2)
        }
    }

    Vector2::new(x, y)
}

fn cast_ray(start: Vector2, dir: Vector2, board: &Board) -> Vec<Vector2> {
    let straight = Straight::new(start, start.add(dir));
    let eps = Vector2::new(f32::signum(straight.dir.x) * EPS, f32::signum(straight.dir.y) * EPS);

    let mut point = next_ray_step(start, &straight);
    let mut points: Vec<Vector2> = vec![];

    let mut dist = point.distance_to(start).powi(2);
    let mut last_dist = dist - 1.0;

    while dist < FAR_CLIPING_PLANE*FAR_CLIPING_PLANE  && dist != last_dist {
        let x = if dir.x > 0.0 { f32::floor(point.x) }
        else { f32::ceil(point.x) - 1.0};

        let y = if dir.y > 0.0 { f32::floor(point.y + eps.y) }
        else { f32::ceil(point.y) - 1.0 };

        let x = f32::max(f32::min(x, board.cols as f32 - 1.0), 0.0) as usize;
        let y = f32::max(f32::min(y, board.rows as f32 - 1.0), 0.0) as usize;
        match board.at(x, y) {
            Cell::EMPTY => {},
            Cell::TranslucentTexture(_) => {
                points.push(point)
            },
            _ => {
                points.push(point);
                break
            },
        }

        point = next_ray_step(point.add(eps), &straight);

        last_dist = dist;
        dist = point.distance_to(start).powi(2);
    }

    points
}

fn get_hitted_cells<'a>(game: &'a Game) -> [Vec<(&'a Cell, Vector2)>; NUM_OF_RAYS] {
    let mut all_cells = [(); NUM_OF_RAYS].map(|_| vec![(&Cell::EMPTY, Vector2::zero())]);

    let half_fov = (FOV/2.0) * std::f32::consts::PI / 180.0;
    let start = game.player.dir.rotated(half_fov);
    let end = game.player.dir.rotated(-half_fov);
    let lerp_amount = end.sub(start).div(NUM_OF_RAYS as f32);

    let mut dir = start;
    for cells in all_cells.iter_mut() {
        let points = cast_ray(game.player.pos, dir, &game.board);
        for point in points.iter() {
            let mut cell = (&Cell::EMPTY, Vector2::zero());
            cell.1 = *point;

            if point.x >= 0.0 && point.x < game.board.cols as f32 && point.y >= 0.0 && point.y < game.board.rows  as f32{

                let x = if dir.x > 0.0 { f32::floor(point.x) }
                else { f32::ceil(point.x) - 1.0 } as usize;
                let y = if dir.y > 0.0 { f32::floor(point.y) }
                else { f32::ceil(point.y) - 1.0} as usize;
                cell.0 = game.board.at(x, y);
            }

            cells.push(cell);
        }

        dir.add_assign(lerp_amount);
    }

    all_cells
}

fn darken_color(color: &Color, dist: f32) -> Color {
    let hsv = color.color_to_hsv();
    Color::color_from_hsv(hsv.x, hsv.y, hsv.z * (1.0 - dist))
}

fn update_controls(d: &RaylibDrawHandle, game: &mut Game) {
    let delta = d.get_frame_time();
    if d.is_key_down(raylib::ffi::KeyboardKey::KEY_W) {
        game.player.move_forward(delta);
    }

    if d.is_key_down(raylib::ffi::KeyboardKey::KEY_S) {
        game.player.move_backward(delta);
    }

    if d.is_key_down(raylib::ffi::KeyboardKey::KEY_A) {
        game.player.turn_left(delta);
    }

    if d.is_key_down(raylib::ffi::KeyboardKey::KEY_D) {
        game.player.turn_right(delta);
    }
}

fn minimap_mouse_event(d: &mut RaylibDrawHandle, mt: &Transform2D, game: &mut Game) {
    let mouse = d.get_mouse_position().sub(mt.offset).div(mt.zoom);

    let x = mouse.x as usize;
    let y = mouse.y as usize;

    if mouse.x >= 0.0 && mouse.y >= 0.0 && mouse.x < game.board.cols as f32 && mouse.y < game.board.rows as f32 {
        if d.is_mouse_button_pressed(raylib::ffi::MouseButton::MOUSE_BUTTON_LEFT) {
            game.player.pos = Vector2::new(x as f32 + 0.5, y as f32 + 0.5);
        }
    }
}

fn render_game(d: &mut RaylibDrawHandle, game: &Game) {
    let mut gt = Transform2D::default();
    gt.zoom.mul_assign(WINDOW_SIZE.x / NUM_OF_RAYS as f32);
    let window_size = WINDOW_SIZE.div(gt.zoom);

    let max_dist = Vector2::new(game.board.cols as f32, game.board.rows as f32).length();
    let hitted_cells = get_hitted_cells(game);

    for (x, cells) in hitted_cells.iter().rev().enumerate() {
        for cell in cells.iter().rev() {
            let dist = cell.1.sub(game.player.pos).dot(game.player.dir);

            let h = (window_size.y / dist) / (2.0 * window_size.y / window_size.x);
            let pos = Vector2::new(x as f32, (window_size.y - h) / 2.0);

            match cell.0 {
                Cell::EMPTY => {},
                Cell::COLOR(color) => {
                    let color = darken_color(color, max_dist);
                    d.draw_rectangle_v(pos.apply(&gt), Vector2::new(1.0, h).apply_zoom(&gt), color);
                },
                Cell::TranslucentTexture(texture) | Cell::TEXTURE(texture) => {
                    let nx = cell.1.x - f32::floor(cell.1.x);
                    let ny = cell.1.y - f32::floor(cell.1.y);

                    let mut tx = texture.width as f32;
                    let mut ty = texture.height as f32;

                    if ny ==  0.0 {
                        tx *= nx;
                        ty *= ny;
                    } else {
                        tx *= ny;
                        ty *= nx;
                    }

                    let th = texture.height as f32;
                    let tw = 1.0;

                    let color = darken_color(&Color::WHITE, dist/max_dist);

                    let source_rec = Rectangle::new(tx, ty, tw, th);
                    let dest_rec = Rectangle::new(pos.x, pos.y, 1.0, h).apply(&gt);

                    d.draw_texture_pro(texture, source_rec, dest_rec, Vector2::zero(), 0.0, color);
                }
            }
        }
    }
}

fn render_player(d: &mut RaylibDrawHandle, mt: &Transform2D, player: &Player) {
    let zoom =  f32::max(mt.zoom.x, mt.zoom.y);
    let pos = player.pos.apply(&mt);

    let half_fov = (FOV/2.0) * std::f32::consts::PI / 180.0;
    let p1 = player.dir.rotated(half_fov);
    let p2 = player.dir.rotated(-half_fov);

    d.draw_triangle(pos, pos.add(p1.apply_zoom(&mt)), pos.add(p2.apply_zoom(&mt)), Color::PURPLE);
    d.draw_circle_v(pos, 0.2 * zoom, Color::RED);
}

fn render_minimap(d: &mut RaylibDrawHandle, mt: &Transform2D,  game: &Game) {
    let board_size = Vector2::new(game.board.cols as f32, game.board.rows as f32);
    d.draw_rectangle_v(Vector2::zero().apply(&mt), board_size.apply_zoom(&mt), Color::BLACK);

    // render grid
    for y in 0..=game.board.rows{
        d.draw_line_v(Vector2::new(0.0, y as f32).apply(&mt), Vector2::new(board_size.x, y as f32).apply(&mt), Color::GRAY);
    }

    for x in 0..=game.board.cols{
        d.draw_line_v(Vector2::new(x as f32, 0.0).apply(&mt), Vector2::new(x as f32, board_size.y).apply(&mt), Color::GRAY);
    }

    // render cells
    for y in 0..game.board.rows{
        for x in 0..game.board.cols{
            let cell = game.board.at(x, y);

            let pos = Vector2::new(x as f32, y as f32).apply(&mt);
            let size = Vector2::one().apply_zoom(&mt);

            match cell {
                Cell::EMPTY => {},
                Cell::COLOR(color) => d.draw_rectangle_v(pos, size, color),
                Cell::TranslucentTexture(texture) | Cell::TEXTURE(texture) => {
                    let source_rec =Rectangle::new(0.0, 0.0, texture.width as f32, texture.height as f32);
                    let dest_rec = Rectangle::new(x as f32, y as f32, 1.0, 1.0).apply(&mt);
                    d.draw_texture_pro(texture, source_rec, dest_rec, Vector2::zero(), 0.0, Color::WHITE);
                },
            }
        }
    }

    render_player(d, &mt, &game.player);
}

fn calulate_minimap_size(board_size: Vector2) -> Vector2 {
    if board_size.x > board_size.y {
        let x = WINDOW_SIZE.x * MINIMAP_ASPECT_RATIO;
        let y = (x / board_size.x) * board_size.y;
        Vector2::new(x, y)
    } else {
        let y = WINDOW_SIZE.y * MINIMAP_ASPECT_RATIO;
        let x = (y / board_size.y) * board_size.x;
        Vector2::new(x, y)
    }
}

fn load_texture(rl: &mut RaylibHandle, thread: &RaylibThread, filename: &str) -> Texture2D {
    match rl.load_texture(&thread, filename) {
        Ok(texture) => texture,
        Err(err) => {
            println!("ERROR: {}", err);
            exit(1);
        },
    }
}

fn main() {
    let (mut rl, thread) = raylib::init()
        .size(WINDOW_SIZE.x as i32, WINDOW_SIZE.y as i32)
        .title("raycasting")
        .build();

    let board = Board::new(10, 10);
    let player = Player::new(0.0, 0.0);
    let mut game = Game { board, player };
    game.player.spd.mul_assign(3.0);
    game.player.turn_spd *= 2.0;

    let board_size = Vector2::new(game.board.cols as f32, game.board.rows as f32);
    let minimap_size = calulate_minimap_size(board_size);
    let margin = Vector2::one().mul(10.0);

    let mut mt = Transform2D::default();
    mt.zoom = minimap_size.div(board_size);
    mt.offset = WINDOW_SIZE.sub(minimap_size).sub(margin);

    let galo_cego = Cell::TEXTURE(load_texture(&mut rl, &thread, "./res/galo-cego.png"));
    let atumalaca = Cell::TEXTURE(load_texture(&mut rl, &thread, "./res/atumalaca.png"));
    let steve_face = Cell::TEXTURE(load_texture(&mut rl, &thread, "./res/steve-face.png"));
    let steve_body = Cell::TranslucentTexture(load_texture(&mut rl, &thread, "./res/steve-body.png"));
    let glass = Cell::TranslucentTexture(load_texture(&mut rl, &thread, "./res/glass.png"));

    game.board.set(5, 5, &Cell::COLOR(Color::BLUE));
    game.board.set(5, 6, &Cell::COLOR(Color::YELLOW));
    game.board.set(5, 4, &Cell::COLOR(Color::RED));
    game.board.set(4, 3, &Cell::COLOR(Color::GREEN));

    game.board.set(1, 7, &galo_cego);
    game.board.set(3, 7, &atumalaca);
    game.board.set(4, 7, &steve_face);
    game.board.set(2, 7, &steve_body);
    game.board.set(4, 4, &glass);


    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);

        update_controls(&d, &mut game);
        minimap_mouse_event(&mut d, &mt, &mut game);

        render_game(&mut d, &game);
        render_minimap(&mut d, &mt, &game);
    }
}
