//! Optional, experimental IPC bridge for a mapped Fabric client API subset.
//! Java mod code stays in a separate JVM. Horizon sends bounded read-only
//! player and world-clock snapshots without waiting for Java, then receives
//! bounded chat, HUD, and player-control actions from the sidecar.

use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::sync::{Mutex, OnceLock};

const ACTION_QUEUE_CAPACITY: usize = 16;
const MAX_BRIDGE_RESPONSE_BYTES: usize = 4096;
const MAX_ACTION_TEXT_BYTES: usize = 1024;
const MAX_ACTIONS_PER_TICK: usize = 4;
const MAX_PLAYER_NAME_BYTES: usize = 128;
const MAX_FABRIC_PLAYER_SPEED: f64 = 4.0;
const MAX_FABRIC_PLAYER_YAW: f32 = 360_000.0;
const MIN_FABRIC_PLAYER_PITCH: f32 = -90.0;
const MAX_FABRIC_PLAYER_PITCH: f32 = 90.0;
const MAX_KEY_CODES_PER_TICK: usize = 64;
const MAX_MINECRAFT_KEY_CODE: u16 = 255;
const MAX_BLOCK_SNAPSHOTS: usize = 27;
const MAX_BLOCK_ID_BYTES: usize = 256;
const MAX_BLOCK_SNAPSHOT_BYTES: usize = 8192;
pub const VANILLA_INPUT_UP: u8 = 1 << 0;
pub const VANILLA_INPUT_DOWN: u8 = 1 << 1;
pub const VANILLA_INPUT_LEFT: u8 = 1 << 2;
pub const VANILLA_INPUT_RIGHT: u8 = 1 << 3;
pub const VANILLA_INPUT_JUMP: u8 = 1 << 4;
pub const VANILLA_INPUT_SHIFT: u8 = 1 << 5;
pub const VANILLA_INPUT_SPRINT: u8 = 1 << 6;
const MAX_VANILLA_INPUT_FLAGS: u8 = VANILLA_INPUT_UP
    | VANILLA_INPUT_DOWN
    | VANILLA_INPUT_LEFT
    | VANILLA_INPUT_RIGHT
    | VANILLA_INPUT_JUMP
    | VANILLA_INPUT_SHIFT
    | VANILLA_INPUT_SPRINT;

pub fn encode_vanilla_input_flags(
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    jump: bool,
    shift: bool,
    sprint: bool,
) -> u8 {
    [
        (up, VANILLA_INPUT_UP),
        (down, VANILLA_INPUT_DOWN),
        (left, VANILLA_INPUT_LEFT),
        (right, VANILLA_INPUT_RIGHT),
        (jump, VANILLA_INPUT_JUMP),
        (shift, VANILLA_INPUT_SHIFT),
        (sprint, VANILLA_INPUT_SPRINT),
    ]
    .into_iter()
    .fold(0, |flags, (pressed, flag)| {
        if pressed { flags | flag } else { flags }
    })
}

static TICK_SENDER: OnceLock<SyncSender<FabricBridgeTick>> = OnceLock::new();
static ACTION_RECEIVER: OnceLock<Mutex<Receiver<FabricBridgeAction>>> = OnceLock::new();
static DISPATCHED_CALLBACKS: AtomicU64 = AtomicU64::new(0);
static BRIDGE_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FabricBridgeTickPhase {
    Start,
    End,
}

#[derive(Clone, Debug, PartialEq)]
struct FabricBridgeTick {
    phase: FabricBridgeTickPhase,
    player: Option<FabricBridgePlayerSnapshot>,
    overworld_clock_time: Option<i64>,
    vanilla_input_flags: u8,
    down_keys: Vec<u16>,
    pressed_keys: Vec<u16>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FabricBridgeAction {
    pub overlay_message: Option<String>,
    pub system_message: Option<String>,
    pub animate: bool,
    pub sprinting: Option<bool>,
    pub player_velocity: Option<[f64; 3]>,
    pub player_rotation: Option<[f32; 2]>,
}

/// Read-only local-player state copied from Horizon's game thread for one
/// asynchronous Fabric client-tick callback.
#[derive(Clone, Debug, PartialEq)]
pub struct FabricBridgePlayerSnapshot {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub velocity_x: f64,
    pub velocity_y: f64,
    pub velocity_z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub health: f32,
    pub sprinting: bool,
    pub on_ground: bool,
    pub game_time: i64,
    pub min_build_height: i32,
    pub max_build_height: i32,
    pub raining: bool,
    pub thundering: bool,
    pub rain_level_previous: f32,
    pub rain_level_current: f32,
    pub thunder_level_previous: f32,
    pub thunder_level_current: f32,
    pub nearby_blocks: Vec<FabricBridgeBlockSnapshot>,
}

/// Bounded read-only block facts captured around the local player on Horizon's game thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FabricBridgeBlockSnapshot {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block_id: String,
    pub is_air: bool,
    pub full_collision: bool,
    pub solid_render: bool,
}

/// Connects the Java sidecar to the native game's non-blocking tick queue.
pub fn attach(stream: TcpStream) -> std::io::Result<()> {
    if TICK_SENDER.get().is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "Fabric bridge is already attached",
        ));
    }
    stream.set_nodelay(true)?;
    let reader_stream = stream.try_clone()?;
    let (sender, receiver) = sync_channel(4);
    let (action_sender, action_receiver) = sync_channel(ACTION_QUEUE_CAPACITY);
    std::thread::Builder::new()
        .name("horizon-fabric-bridge".to_owned())
        .spawn(move || bridge_worker(stream, reader_stream, receiver, action_sender))?;
    TICK_SENDER.set(sender).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "Fabric bridge is already attached",
        )
    })?;
    ACTION_RECEIVER.set(Mutex::new(action_receiver)).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "Fabric bridge action receiver is already attached",
        )
    })?;
    BRIDGE_ACTIVE.store(true, Ordering::Release);
    Ok(())
}

/// Whether the Java sidecar is still accepting tick snapshots.
pub fn is_attached() -> bool {
    BRIDGE_ACTIVE.load(Ordering::Acquire)
}

/// Called from the client game's native tick. A busy sidecar drops excess
/// snapshots and callbacks together; it never makes the game thread wait for Java.
pub fn dispatch_start_tick(
    player: Option<FabricBridgePlayerSnapshot>,
    overworld_clock_time: Option<i64>,
    down_keys: &[u16],
    pressed_keys: &[u16],
    vanilla_input_flags: u8,
) -> bool {
    dispatch_tick(
        FabricBridgeTickPhase::Start,
        player,
        overworld_clock_time,
        vanilla_input_flags,
        down_keys,
        pressed_keys,
    )
}

pub fn dispatch_end_tick(
    player: Option<FabricBridgePlayerSnapshot>,
    overworld_clock_time: Option<i64>,
    down_keys: &[u16],
    vanilla_input_flags: u8,
) -> bool {
    dispatch_tick(
        FabricBridgeTickPhase::End,
        player,
        overworld_clock_time,
        vanilla_input_flags,
        down_keys,
        &[],
    )
}

fn dispatch_tick(
    phase: FabricBridgeTickPhase,
    player: Option<FabricBridgePlayerSnapshot>,
    overworld_clock_time: Option<i64>,
    vanilla_input_flags: u8,
    down_keys: &[u16],
    pressed_keys: &[u16],
) -> bool {
    if !is_attached() {
        return false;
    }
    let Some(sender) = TICK_SENDER.get() else {
        return false;
    };
    if vanilla_input_flags & !MAX_VANILLA_INPUT_FLAGS != 0
        || down_keys.len() > MAX_KEY_CODES_PER_TICK
        || pressed_keys.len() > MAX_KEY_CODES_PER_TICK
        || down_keys.iter().chain(pressed_keys).any(|key| *key > MAX_MINECRAFT_KEY_CODE)
    {
        return false;
    }
    try_queue_tick(sender, FabricBridgeTick {
        phase,
        player,
        overworld_clock_time,
        vanilla_input_flags,
        down_keys: down_keys.to_vec(),
        pressed_keys: pressed_keys.to_vec(),
    })
}

/// Maps Horizon's physical key names to Minecraft 26.3 keyboard (SDL/HID) values.
/// These are physical positions, independent of the active keyboard layout.
pub fn minecraft_keyboard_key_code(key: &str) -> Option<u16> {
    let key = key.strip_prefix("key.keyboard.")?;
    if key.len() == 1 {
        let byte = key.as_bytes()[0];
        if byte.is_ascii_lowercase() {
            return Some(4 + u16::from(byte - b'a'));
        }
        if (b'1'..=b'9').contains(&byte) {
            return Some(30 + u16::from(byte - b'1'));
        }
        if byte == b'0' {
            return Some(39);
        }
    }
    if let Some(number) = key.strip_prefix('f').and_then(|number| number.parse::<u16>().ok()) {
        return match number {
            1..=12 => Some(57 + number),
            13..=24 => Some(91 + number),
            _ => None,
        };
    }
    if let Some(number) = key.strip_prefix("keypad.").and_then(|number| number.parse::<u16>().ok()) {
        return match number {
            1..=9 => Some(88 + number),
            0 => Some(98),
            _ => None,
        };
    }
    Some(match key {
        "space" => 44,
        "apostrophe" => 52,
        "comma" => 54,
        "minus" => 45,
        "period" => 55,
        "slash" => 56,
        "semicolon" => 51,
        "equal" => 46,
        "left.bracket" => 47,
        "backslash" => 49,
        "right.bracket" => 48,
        "grave.accent" => 53,
        "world.1" => 100,
        "international1" => 135,
        "international3" => 137,
        "lang1" => 144,
        "lang2" => 145,
        "lang3" => 146,
        "lang4" => 147,
        "lang5" => 148,
        "help" => 117,
        "select" => 119,
        "again" => 121,
        "undo" => 122,
        "cut" => 123,
        "copy" => 124,
        "paste" => 125,
        "find" => 126,
        "escape" => 41,
        "enter" => 40,
        "tab" => 43,
        "backspace" => 42,
        "insert" => 73,
        "delete" => 76,
        "right" => 79,
        "left" => 80,
        "down" => 81,
        "up" => 82,
        "page.up" => 75,
        "page.down" => 78,
        "home" => 74,
        "end" => 77,
        "caps.lock" => 57,
        "num.lock" => 83,
        "keypad.divide" => 84,
        "keypad.multiply" => 85,
        "keypad.subtract" => 86,
        "keypad.add" => 87,
        "keypad.enter" => 88,
        "keypad.period" => 99,
        "keypad.equal" => 103,
        "keypad.comma" => 220,
        "print.screen" => 70,
        "pause" => 72,
        "scroll.lock" => 71,
        "left.control" => 224,
        "left.shift" => 225,
        "left.alt" => 226,
        "right.control" => 228,
        "right.shift" => 229,
        "right.alt" => 230,
        "left.win" => 227,
        "right.win" => 231,
        "menu" => 101,
        _ => return None,
    })
}

/// Number of acknowledged Fabric client-tick callbacks in this process.
pub fn dispatched_callbacks() -> u64 {
    DISPATCHED_CALLBACKS.load(Ordering::Relaxed)
}

/// Takes native actions emitted by Fabric callbacks. The game thread drains a
/// small fixed batch so Java callbacks never block or mutate Horizon directly.
pub fn take_actions() -> Vec<FabricBridgeAction> {
    let Some(receiver) = ACTION_RECEIVER.get() else {
        return Vec::new();
    };
    let Ok(receiver) = receiver.lock() else {
        return Vec::new();
    };
    let mut actions = Vec::with_capacity(MAX_ACTIONS_PER_TICK);
    while actions.len() < MAX_ACTIONS_PER_TICK {
        match receiver.try_recv() {
            Ok(action) => actions.push(action),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
        }
    }
    actions
}

fn bridge_worker(
    mut writer: TcpStream,
    reader_stream: TcpStream,
    receiver: std::sync::mpsc::Receiver<FabricBridgeTick>,
    action_sender: SyncSender<FabricBridgeAction>,
) {
    let mut reader = BufReader::new(reader_stream);
    while let Ok(tick) = receiver.recv() {
        let Some(command) = encode_tick_command(&tick) else {
            eprintln!("Horizon Fabric bridge: invalid local game or input snapshot; dropping this callback tick.");
            continue;
        };
        if writer.write_all(command.as_bytes()).is_err() || writer.flush().is_err() {
            eprintln!("Horizon Fabric bridge: Java sidecar disconnected.");
            break;
        }
        let mut response = Vec::new();
        match read_bounded_line(&mut reader, &mut response, MAX_BRIDGE_RESPONSE_BYTES) {
            Ok(0) | Err(_) => {
                eprintln!("Horizon Fabric bridge: Java sidecar stopped responding.");
                break;
            }
            Ok(_) => {
                let Ok(response) = std::str::from_utf8(&response) else {
                    eprintln!("Horizon Fabric bridge: Java sidecar returned a malformed response.");
                    break;
                };
                match parse_tick_response(response) {
                    Some((callbacks, action)) => {
                        DISPATCHED_CALLBACKS.store(callbacks, Ordering::Relaxed);
                        if callbacks == 1 {
                            eprintln!(
                                "Horizon Fabric bridge: first Fabric client-tick callback dispatched by the native client hook."
                            );
                        }
                        if let Some(action) = action {
                            match action_sender.try_send(action) {
                                Ok(()) => {}
                                Err(TrySendError::Full(_)) => {
                                    eprintln!("Horizon Fabric bridge: native action queue is full; dropping an action-bar update.");
                                }
                                Err(TrySendError::Disconnected(_)) => break,
                            }
                        }
                    }
                    None if response.trim() == "unsupported" => {
                        eprintln!("Horizon Fabric bridge: the installed Fabric API is missing a mapped client-tick event; Java callbacks stopped.");
                        break;
                    }
                    None if response.trim() == "error" => {
                        eprintln!("Horizon Fabric bridge: Java mod callback failed; callback dispatch has been stopped.");
                        break;
                    }
                    None => {
                        eprintln!("Horizon Fabric bridge: Java sidecar returned an invalid action response.");
                        break;
                    }
                }
            }
        }
    }
    BRIDGE_ACTIVE.store(false, Ordering::Release);
}

fn encode_tick_command(tick: &FabricBridgeTick) -> Option<String> {
    let phase = match tick.phase {
        FabricBridgeTickPhase::Start => "start",
        FabricBridgeTickPhase::End => "end",
    };
    let clock_time = tick
        .overworld_clock_time
        .map_or_else(|| "-".to_owned(), |time| time.to_string());
    let vanilla_input_flags = tick.vanilla_input_flags;
    if vanilla_input_flags & !MAX_VANILLA_INPUT_FLAGS != 0 {
        return None;
    }
    let down_keys = encode_key_codes(&tick.down_keys)?;
    let pressed_keys = encode_key_codes(&tick.pressed_keys)?;
    let Some(player) = tick.player.as_ref() else {
        return Some(format!(
            "{phase}\t-\t{clock_time}\t{vanilla_input_flags}\t{down_keys}\t{pressed_keys}\t-\n"
        ));
    };
    if player.name.is_empty()
        || player.name.len() > MAX_PLAYER_NAME_BYTES
        || player.name.chars().any(char::is_control)
        || !player.x.is_finite()
        || !player.y.is_finite()
        || !player.z.is_finite()
        || !player.velocity_x.is_finite()
        || !player.velocity_y.is_finite()
        || !player.velocity_z.is_finite()
        || !player.yaw.is_finite()
        || !player.pitch.is_finite()
        || !player.health.is_finite()
        || player.min_build_height >= player.max_build_height
        || player.min_build_height < -4096
        || player.max_build_height > 4096
        || !player.rain_level_previous.is_finite()
        || !(0.0..=1.0).contains(&player.rain_level_previous)
        || !player.rain_level_current.is_finite()
        || !(0.0..=1.0).contains(&player.rain_level_current)
        || !player.thunder_level_previous.is_finite()
        || !(0.0..=1.0).contains(&player.thunder_level_previous)
        || !player.thunder_level_current.is_finite()
        || !(0.0..=1.0).contains(&player.thunder_level_current)
    {
        return None;
    }
    let nearby_blocks = encode_nearby_blocks(
        &player.nearby_blocks,
        player.x,
        player.y,
        player.z,
    )?;
    let mut name_hex = String::with_capacity(player.name.len() * 2);
    for byte in player.name.as_bytes() {
        use std::fmt::Write as _;
        write!(&mut name_hex, "{byte:02x}").ok()?;
    }
    Some(format!(
        "{phase}\t{name_hex}\t{x}\t{y}\t{z}\t{velocity_x}\t{velocity_y}\t{velocity_z}\t{yaw}\t{pitch}\t{health}\t{sprinting}\t{on_ground}\t{clock_time}\t{game_time}\t{min_build_height}\t{max_build_height}\t{raining}\t{thundering}\t{rain_level_previous}\t{rain_level_current}\t{thunder_level_previous}\t{thunder_level_current}\t{vanilla_input_flags}\t{down_keys}\t{pressed_keys}\t{nearby_blocks}\n",
        x = player.x,
        y = player.y,
        z = player.z,
        velocity_x = player.velocity_x,
        velocity_y = player.velocity_y,
        velocity_z = player.velocity_z,
        yaw = player.yaw,
        pitch = player.pitch,
        health = player.health,
        sprinting = player.sprinting,
        on_ground = player.on_ground,
        game_time = player.game_time,
        min_build_height = player.min_build_height,
        max_build_height = player.max_build_height,
        raining = player.raining,
        thundering = player.thundering,
        rain_level_previous = player.rain_level_previous,
        rain_level_current = player.rain_level_current,
        thunder_level_previous = player.thunder_level_previous,
        thunder_level_current = player.thunder_level_current,
        nearby_blocks = nearby_blocks,
    ))
}

fn encode_nearby_blocks(
    blocks: &[FabricBridgeBlockSnapshot],
    player_x: f64,
    player_y: f64,
    player_z: f64,
) -> Option<String> {
    if blocks.len() > MAX_BLOCK_SNAPSHOTS
        || [player_x, player_y, player_z]
            .iter()
            .any(|value| *value < f64::from(i32::MIN) || *value > f64::from(i32::MAX))
    {
        return None;
    }
    let center = [player_x.floor() as i32, player_y.floor() as i32, player_z.floor() as i32];
    let mut encoded = Vec::with_capacity(blocks.len());
    for block in blocks {
        if block.block_id.len() > MAX_BLOCK_ID_BYTES
            || !valid_block_id(&block.block_id)
            || [block.x, block.y, block.z]
                .into_iter()
                .zip(center)
                .any(|(position, origin)| (i64::from(position) - i64::from(origin)).abs() > 1)
            || blocks[..encoded.len()].iter().any(|previous| {
                previous.x == block.x && previous.y == block.y && previous.z == block.z
            })
        {
            return None;
        }
        encoded.push(format!(
            "{},{},{},{},{},{},{}",
            block.x,
            block.y,
            block.z,
            block.block_id,
            u8::from(block.is_air),
            u8::from(block.full_collision),
            u8::from(block.solid_render),
        ));
    }
    let snapshot = if encoded.is_empty() { "-".to_owned() } else { encoded.join(";") };
    (snapshot.len() <= MAX_BLOCK_SNAPSHOT_BYTES).then_some(snapshot)
}

fn valid_block_id(id: &str) -> bool {
    let Some((namespace, path)) = id.split_once(':') else {
        return false;
    };
    !namespace.is_empty()
        && !path.is_empty()
        && namespace
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_.-".contains(&byte))
        && path.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_./-".contains(&byte)
        })
}

fn encode_key_codes(codes: &[u16]) -> Option<String> {
    if codes.len() > MAX_KEY_CODES_PER_TICK || codes.iter().any(|key| *key > MAX_MINECRAFT_KEY_CODE) {
        return None;
    }
    if codes.is_empty() {
        return Some("-".to_owned());
    }
    Some(codes.iter().map(u16::to_string).collect::<Vec<_>>().join(","))
}

fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    output: &mut Vec<u8>,
    limit: usize,
) -> io::Result<usize> {
    output.clear();
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            return Ok(output.len());
        }
        let bytes_to_consume = buffer
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(buffer.len(), |index| index + 1);
        if output.len().saturating_add(bytes_to_consume) > limit {
            reader.consume(bytes_to_consume);
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Fabric bridge response is too large",
            ));
        }
        let line_ended = buffer.get(bytes_to_consume - 1) == Some(&b'\n');
        output.extend_from_slice(&buffer[..bytes_to_consume]);
        reader.consume(bytes_to_consume);
        if line_ended {
            return Ok(output.len());
        }
    }
}

fn parse_tick_response(response: &str) -> Option<(u64, Option<FabricBridgeAction>)> {
    let response = response.trim_end_matches(['\r', '\n']);
    let mut fields = response.split('\t');
    if fields.next()? != "tick" {
        return None;
    }
    let ticks = fields.next()?.parse().ok()?;
    let overlay = match fields.next()? {
        "-" => None,
        value => Some(parse_overlay_action(value)?),
    };
    let sprinting = match fields.next()? {
        "-" => None,
        "S0" => Some(false),
        "S1" => Some(true),
        _ => return None,
    };
    let player_velocity = match fields.next()? {
        "-" => None,
        value => Some(parse_player_velocity_action(value)?),
    };
    let system_message = match fields.next()? {
        "-" => None,
        value => Some(parse_system_message_action(value)?),
    };
    let player_rotation = match fields.next()? {
        "-" => None,
        value => Some(parse_player_rotation_action(value)?),
    };
    if fields.next().is_some() {
        return None;
    }
    let action = (overlay.is_some()
        || sprinting.is_some()
        || player_velocity.is_some()
        || system_message.is_some()
        || player_rotation.is_some())
        .then(|| FabricBridgeAction {
            overlay_message: overlay.as_ref().map(|(message, _)| message.clone()),
            system_message,
            animate: overlay.is_some_and(|(_, animate)| animate),
            sprinting,
            player_velocity,
            player_rotation,
        });
    Some((ticks, action))
}

fn parse_player_velocity_action(encoded: &str) -> Option<[f64; 3]> {
    let mut fields = encoded.strip_prefix("V:")?.split(',');
    let velocity = [
        fields.next()?.parse::<f64>().ok()?,
        fields.next()?.parse::<f64>().ok()?,
        fields.next()?.parse::<f64>().ok()?,
    ];
    if fields.next().is_some() || velocity.iter().any(|component: &f64| !component.is_finite()) {
        return None;
    }
    let speed_squared = velocity.iter().map(|component| component * component).sum::<f64>();
    (speed_squared.is_finite() && speed_squared <= MAX_FABRIC_PLAYER_SPEED * MAX_FABRIC_PLAYER_SPEED)
        .then_some(velocity)
}

fn parse_overlay_action(encoded: &str) -> Option<(String, bool)> {
    let (animate, hex) = match encoded.strip_prefix("A0:") {
        Some(hex) => (false, hex),
        None => (true, encoded.strip_prefix("A1:")?),
    };
    Some((decode_action_text(hex)?, animate))
}

fn parse_system_message_action(encoded: &str) -> Option<String> {
    decode_action_text(encoded.strip_prefix("M:")?)
}

fn parse_player_rotation_action(encoded: &str) -> Option<[f32; 2]> {
    let mut fields = encoded.strip_prefix("R:")?.split(',');
    let yaw = fields.next()?.parse::<f32>().ok()?;
    let pitch = fields.next()?.parse::<f32>().ok()?;
    if fields.next().is_some()
        || !yaw.is_finite()
        || yaw.abs() > MAX_FABRIC_PLAYER_YAW
        || !pitch.is_finite()
        || !(MIN_FABRIC_PLAYER_PITCH..=MAX_FABRIC_PLAYER_PITCH).contains(&pitch)
    {
        return None;
    }
    Some([yaw, pitch])
}

fn decode_action_text(hex: &str) -> Option<String> {
    if hex.len() % 2 != 0 || hex.len() / 2 > MAX_ACTION_TEXT_BYTES {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for pair in hex.as_bytes().chunks_exact(2) {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        bytes.push((high << 4) | low);
    }
    let message = String::from_utf8(bytes).ok()?;
    (!message.chars().any(char::is_control)).then_some(message)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn try_queue_tick<T>(sender: &SyncSender<T>, tick: T) -> bool {
    match sender.try_send(tick) {
        Ok(()) => true,
        Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => false,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::sync_channel;

    use super::{
        FabricBridgeAction, FabricBridgeBlockSnapshot, FabricBridgePlayerSnapshot, FabricBridgeTick,
        FabricBridgeTickPhase, MAX_VANILLA_INPUT_FLAGS, VANILLA_INPUT_DOWN, VANILLA_INPUT_JUMP,
        VANILLA_INPUT_LEFT, VANILLA_INPUT_RIGHT, VANILLA_INPUT_SHIFT, VANILLA_INPUT_SPRINT,
        VANILLA_INPUT_UP, encode_tick_command, encode_vanilla_input_flags, is_attached,
        parse_tick_response, try_queue_tick, minecraft_keyboard_key_code,
    };

    #[test]
    fn parses_mapped_native_action_responses_and_rejects_bad_payloads() {
        assert_eq!(
            parse_tick_response("tick\t17\tA1:48656c6c6f\t-\t-\t-\t-\n"),
            Some((
                17,
                Some(FabricBridgeAction {
                    overlay_message: Some("Hello".to_owned()),
                    system_message: None,
                    animate: true,
                    sprinting: None,
                    player_velocity: None,
                    player_rotation: None,
                })
            ))
        );
        assert_eq!(
            parse_tick_response("tick\t18\t-\tS1\t-\t-\t-\n"),
            Some((
                18,
                Some(FabricBridgeAction {
                    overlay_message: None,
                    system_message: None,
                    animate: false,
                    sprinting: Some(true),
                    player_velocity: None,
                    player_rotation: None,
                })
            ))
        );
        assert_eq!(parse_tick_response("tick\t19\t-\t-\t-\t-\t-\n"), Some((19, None)));
        assert_eq!(
            parse_tick_response("tick\t20\t-\tS0\tV:0.0,0.5,0.0\t-\t-\n"),
            Some((
                20,
                Some(FabricBridgeAction {
                    overlay_message: None,
                    system_message: None,
                    animate: false,
                    sprinting: Some(false),
                    player_velocity: Some([0.0, 0.5, 0.0]),
                    player_rotation: None,
                })
            ))
        );
        assert_eq!(
            parse_tick_response("tick\t21\t-\t-\t-\tM:4869\t-\n"),
            Some((
                21,
                Some(FabricBridgeAction {
                    overlay_message: None,
                    system_message: Some("Hi".to_owned()),
                    animate: false,
                    sprinting: None,
                    player_velocity: None,
                    player_rotation: None,
                })
            ))
        );
        assert_eq!(
            parse_tick_response("tick\t22\t-\t-\t-\t-\tR:127.5,-15.0\n"),
            Some((
                22,
                Some(FabricBridgeAction {
                    overlay_message: None,
                    system_message: None,
                    animate: false,
                    sprinting: None,
                    player_velocity: None,
                    player_rotation: Some([127.5, -15.0]),
                })
            ))
        );
        assert_eq!(
            parse_tick_response("tick\t23\t-\t-\t-\t-\tR:-360000.0,90.0\n")
                .and_then(|(_, action)| action)
                .and_then(|action| action.player_rotation),
            Some([-360_000.0, 90.0])
        );
        assert_eq!(parse_tick_response("tick\t24\t-\t-\tV:NaN,0.0,0.0\t-\t-\n"), None);
        assert_eq!(parse_tick_response("tick\t25\t-\t-\tV:3.0,3.0,0.0\t-\t-\n"), None);
        assert_eq!(parse_tick_response("tick\t26\t-\t-\t-\tM:ff\t-\n"), None);
        assert_eq!(parse_tick_response("tick\t27\t-\t-\t-\t-\tR:NaN,0.0\n"), None);
        assert_eq!(parse_tick_response("tick\t28\t-\t-\t-\t-\tR:360001.0,0.0\n"), None);
        assert_eq!(parse_tick_response("tick\t29\t-\t-\t-\t-\tR:0.0,90.1\n"), None);
        assert_eq!(parse_tick_response("tick\t30\t-\t-\t-\t-\tR:0.0,0.0,5.0\n"), None);
        assert_eq!(parse_tick_response("tick\t17\tA0:ff\t-\t-\t-\t-\n"), None);
        assert_eq!(parse_tick_response("tick\t17\tA0:0a\t-\t-\t-\t-\n"), None);
        assert_eq!(parse_tick_response("tick\t17\t-\tS2\t-\t-\t-\n"), None);
        assert_eq!(parse_tick_response("tick\t17\t-\tS0\textra\textra\textra\textra\n"), None);
        assert_eq!(parse_tick_response("unsupported\n"), None);
        assert_eq!(parse_tick_response("tick\tnope\t-\t-\t-\t-\t-\n"), None);
    }

    #[test]
    fn drops_excess_ticks_without_waiting_for_the_java_sidecar() {
        let (sender, receiver) = sync_channel(2);

        assert!(try_queue_tick(&sender, ()));
        assert!(try_queue_tick(&sender, ()));
        assert!(!try_queue_tick(&sender, ()));
        assert_eq!(receiver.try_iter().count(), 2);
    }

    #[test]
    fn bridge_stays_inactive_until_a_sidecar_is_attached() {
        assert!(!is_attached());
    }

    #[test]
    fn encodes_bounded_read_only_player_motion_and_orientation_snapshots_for_each_tick_phase() {
        let player = FabricBridgePlayerSnapshot {
            name: "Tester_2".to_owned(),
            x: 1.25,
            y: 64.0,
            z: -3.5,
            velocity_x: 0.15,
            velocity_y: -0.08,
            velocity_z: 0.3,
            yaw: 37.5,
            pitch: -12.5,
            health: 18.5,
            sprinting: true,
            on_ground: false,
            game_time: 1234,
            min_build_height: -64,
            max_build_height: 320,
            raining: true,
            thundering: false,
            rain_level_previous: 0.25,
            rain_level_current: 0.5,
            thunder_level_previous: 0.1,
            thunder_level_current: 0.2,
            nearby_blocks: vec![
                FabricBridgeBlockSnapshot {
                    x: 1,
                    y: 64,
                    z: -4,
                    block_id: "minecraft:stone".to_owned(),
                    is_air: false,
                    full_collision: true,
                    solid_render: true,
                },
                FabricBridgeBlockSnapshot {
                    x: 1,
                    y: 63,
                    z: -4,
                    block_id: "minecraft:bedrock".to_owned(),
                    is_air: false,
                    full_collision: true,
                    solid_render: true,
                },
            ],
        };
        assert_eq!(
            encode_tick_command(&FabricBridgeTick {
                phase: FabricBridgeTickPhase::Start,
                player: Some(player.clone()),
                overworld_clock_time: Some(6000),
                vanilla_input_flags: 0b0010_0001,
                down_keys: vec![19],
                pressed_keys: vec![19],
            }).as_deref(),
            Some("start\t5465737465725f32\t1.25\t64\t-3.5\t0.15\t-0.08\t0.3\t37.5\t-12.5\t18.5\ttrue\tfalse\t6000\t1234\t-64\t320\ttrue\tfalse\t0.25\t0.5\t0.1\t0.2\t33\t19\t19\t1,64,-4,minecraft:stone,0,1,1;1,63,-4,minecraft:bedrock,0,1,1\n")
        );
        assert_eq!(
            encode_tick_command(&FabricBridgeTick {
                phase: FabricBridgeTickPhase::End,
                player: Some(player.clone()),
                overworld_clock_time: Some(6001),
                vanilla_input_flags: 0,
                down_keys: vec![19],
                pressed_keys: vec![],
            }).as_deref(),
            Some("end\t5465737465725f32\t1.25\t64\t-3.5\t0.15\t-0.08\t0.3\t37.5\t-12.5\t18.5\ttrue\tfalse\t6001\t1234\t-64\t320\ttrue\tfalse\t0.25\t0.5\t0.1\t0.2\t0\t19\t-\t1,64,-4,minecraft:stone,0,1,1;1,63,-4,minecraft:bedrock,0,1,1\n")
        );
        assert_eq!(
            encode_tick_command(&FabricBridgeTick {
                phase: FabricBridgeTickPhase::Start,
                player: None,
                overworld_clock_time: None,
                vanilla_input_flags: 0,
                down_keys: vec![],
                pressed_keys: vec![],
            }).as_deref(),
            Some("start\t-\t-\t0\t-\t-\t-\n")
        );
        assert_eq!(
            encode_tick_command(&FabricBridgeTick {
                phase: FabricBridgeTickPhase::End,
                player: None,
                overworld_clock_time: Some(12000),
                vanilla_input_flags: 0b1000_0000,
                down_keys: vec![19],
                pressed_keys: vec![],
            }).as_deref(),
            None
        );

        let mut invalid = player.clone();
        invalid.x = f64::NAN;
        assert_eq!(encode_tick_command(&FabricBridgeTick {
            phase: FabricBridgeTickPhase::End,
            player: Some(invalid.clone()),
            overworld_clock_time: Some(6000),
            vanilla_input_flags: 0,
            down_keys: vec![],
            pressed_keys: vec![],
        }), None);
        invalid.x = 0.0;
        invalid.velocity_y = f64::NAN;
        assert_eq!(encode_tick_command(&FabricBridgeTick {
            phase: FabricBridgeTickPhase::End,
            player: Some(invalid.clone()),
            overworld_clock_time: Some(6000),
            vanilla_input_flags: 0,
            down_keys: vec![],
            pressed_keys: vec![],
        }), None);
        invalid.velocity_y = 0.0;
        invalid.yaw = f32::INFINITY;
        assert_eq!(encode_tick_command(&FabricBridgeTick {
            phase: FabricBridgeTickPhase::End,
            player: Some(invalid.clone()),
            overworld_clock_time: Some(6000),
            vanilla_input_flags: 0,
            down_keys: vec![],
            pressed_keys: vec![],
        }), None);
        invalid.yaw = 0.0;
        invalid.name = "name\nwith-control".to_owned();
        assert_eq!(encode_tick_command(&FabricBridgeTick {
            phase: FabricBridgeTickPhase::End,
            player: Some(invalid),
            overworld_clock_time: Some(6000),
            vanilla_input_flags: 0,
            down_keys: vec![],
            pressed_keys: vec![],
        }), None);
        let mut invalid_blocks = player.clone();
        invalid_blocks.nearby_blocks[0].x = 3;
        assert_eq!(encode_tick_command(&FabricBridgeTick {
            phase: FabricBridgeTickPhase::End,
            player: Some(invalid_blocks),
            overworld_clock_time: Some(6000),
            vanilla_input_flags: 0,
            down_keys: vec![],
            pressed_keys: vec![],
        }), None, "block snapshots outside the local 3x3x3 region must be rejected");
        let mut duplicate_blocks = player.clone();
        duplicate_blocks.nearby_blocks[1] = duplicate_blocks.nearby_blocks[0].clone();
        assert_eq!(encode_tick_command(&FabricBridgeTick {
            phase: FabricBridgeTickPhase::End,
            player: Some(duplicate_blocks),
            overworld_clock_time: Some(6000),
            vanilla_input_flags: 0,
            down_keys: vec![],
            pressed_keys: vec![],
        }), None, "duplicate block positions must be rejected");
        let mut invalid_block_id = player.clone();
        invalid_block_id.nearby_blocks[0].block_id = "Minecraft:Stone".to_owned();
        assert_eq!(encode_tick_command(&FabricBridgeTick {
            phase: FabricBridgeTickPhase::End,
            player: Some(invalid_block_id),
            overworld_clock_time: Some(6000),
            vanilla_input_flags: 0,
            down_keys: vec![],
            pressed_keys: vec![],
        }), None, "invalid block identifiers must be rejected");
    }

    #[test]
    fn maps_supported_physical_keyboard_keys_to_minecraft_key_values() {
        assert_eq!(minecraft_keyboard_key_code("key.keyboard.a"), Some(4));
        assert_eq!(minecraft_keyboard_key_code("key.keyboard.p"), Some(19));
        assert_eq!(minecraft_keyboard_key_code("key.keyboard.w"), Some(26));
        assert_eq!(minecraft_keyboard_key_code("key.keyboard.1"), Some(30));
        assert_eq!(minecraft_keyboard_key_code("key.keyboard.0"), Some(39));
        assert_eq!(minecraft_keyboard_key_code("key.keyboard.space"), Some(44));
        assert_eq!(minecraft_keyboard_key_code("key.keyboard.keypad.enter"), Some(88));
        assert_eq!(minecraft_keyboard_key_code("key.keyboard.keypad.0"), Some(98));
        assert_eq!(minecraft_keyboard_key_code("key.keyboard.f12"), Some(69));
        assert_eq!(minecraft_keyboard_key_code("key.keyboard.f13"), Some(104));
        assert_eq!(minecraft_keyboard_key_code("key.keyboard.left"), Some(80));
        assert_eq!(minecraft_keyboard_key_code("key.keyboard.left.control"), Some(224));
        assert_eq!(minecraft_keyboard_key_code("key.keyboard.world.1"), Some(100));
        assert_eq!(minecraft_keyboard_key_code("key.keyboard.world.2"), None);
        assert_eq!(minecraft_keyboard_key_code("key.keyboard.unknown"), None);
    }

    #[test]
    fn encodes_each_semantic_vanilla_movement_control() {
        for (controls, expected) in [
            ([true, false, false, false, false, false, false], VANILLA_INPUT_UP),
            ([false, true, false, false, false, false, false], VANILLA_INPUT_DOWN),
            ([false, false, true, false, false, false, false], VANILLA_INPUT_LEFT),
            ([false, false, false, true, false, false, false], VANILLA_INPUT_RIGHT),
            ([false, false, false, false, true, false, false], VANILLA_INPUT_JUMP),
            ([false, false, false, false, false, true, false], VANILLA_INPUT_SHIFT),
            ([false, false, false, false, false, false, true], VANILLA_INPUT_SPRINT),
        ] {
            let [up, down, left, right, jump, shift, sprint] = controls;
            assert_eq!(
                encode_vanilla_input_flags(up, down, left, right, jump, shift, sprint),
                expected
            );
        }
        assert_eq!(
            encode_vanilla_input_flags(true, true, true, true, true, true, true),
            MAX_VANILLA_INPUT_FLAGS
        );
        assert_eq!(
            encode_vanilla_input_flags(false, false, false, false, false, false, false),
            0
        );
    }
}
