#[path = "../../horizon-upstream/crates/horizon-client/src/fabric_bridge.rs"]
mod fabric_bridge;

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    assert_eq!(fabric_bridge::minecraft_keyboard_key_code("key.keyboard.p"), Some(19));
    assert_eq!(
        fabric_bridge::encode_vanilla_input_flags(true, true, true, true, true, true, true),
        0x7f,
        "the semantic movement input bits are not all represented in the bridge protocol"
    );
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    assert!(
        arguments.len() == 4 || arguments.len() == 5,
        "expected JAVA, helper classes, probe JAR, project root; or JAVA, bridge JAR, Loader libraries, probe JAR, project root"
    );

    if arguments.len() == 5 {
        prepare_real_loader_mod(&arguments);
    }

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .expect("the Rust bridge test could not bind its loopback socket");
    listener.set_nonblocking(true).expect("could not configure loopback socket");
    let port = listener.local_addr().expect("could not read loopback port").port();
    let mut helper = JavaHelper(spawn_helper(&arguments, port));

    let connection_deadline = Instant::now() + Duration::from_secs(10);
    let (stream, _) = loop {
        match listener.accept() {
            Ok(connection) => break connection,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if let Some(status) = helper.0.try_wait().expect("could not inspect Java helper") {
                    panic!("Java helper exited before connecting: {status}");
                }
                assert!(
                    Instant::now() < connection_deadline,
                    "Java helper did not connect to Rust"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("Rust bridge accept failed: {error}"),
        }
    };

    let mut shutdown_stream = stream.try_clone().expect("could not clone bridge socket");
    let mut handshake_reader = BufReader::new(
        stream
            .try_clone()
            .expect("could not clone bridge socket"),
    );
    handshake_reader
        .get_ref()
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("could not bound the Java handshake wait");
    let mut handshake = String::new();
    handshake_reader.read_line(&mut handshake).expect("could not read Java handshake");
    handshake_reader
        .get_ref()
        .set_read_timeout(None)
        .expect("could not clear the handshake timeout");
    assert_eq!(handshake.trim(), "ready", "Java helper sent an unexpected handshake");
    fabric_bridge::attach(stream).expect("could not attach the production Rust bridge module");

    fabric_bridge::dispatch_start_tick(None, None, &[], &[], 0);
    let start_action = wait_for_action(Duration::from_secs(5));
    assert_eq!(start_action.overlay_message.as_deref(), Some("Horizon Fabric START_CLIENT_TICK"));
    assert!(!start_action.animate);
    fabric_bridge::dispatch_end_tick(None, None, &[], 0);
    let end_action = wait_for_action(Duration::from_secs(5));
    assert_eq!(end_action.overlay_message.as_deref(), Some("Horizon Fabric END_CLIENT_TICK"));
    assert!(!end_action.animate);

    let snapshot = Some(fabric_bridge::FabricBridgePlayerSnapshot {
        name: "Tester_2".to_owned(),
        x: 1.25,
        y: 64.0,
        z: -3.5,
        velocity_x: 0.3,
        velocity_y: 0.4,
        velocity_z: 0.0,
        yaw: 37.5,
        pitch: -12.5,
        health: 18.5,
        sprinting: true,
        on_ground: false,
        game_time: 300,
        min_build_height: -64,
        max_build_height: 320,
        raining: true,
        thundering: false,
        rain_level_previous: 0.25,
        rain_level_current: 0.5,
        thunder_level_previous: 0.1,
        thunder_level_current: 0.2,
        nearby_blocks: vec![
            fabric_bridge::FabricBridgeBlockSnapshot {
                x: 1,
                y: 64,
                z: -4,
                block_id: "minecraft:stone".to_owned(),
                is_air: false,
                full_collision: true,
                solid_render: true,
            },
            fabric_bridge::FabricBridgeBlockSnapshot {
                x: 1,
                y: 63,
                z: -4,
                block_id: "minecraft:bedrock".to_owned(),
                is_air: false,
                full_collision: true,
                solid_render: true,
            },
        ],
    });
    fabric_bridge::dispatch_start_tick(snapshot.clone(), Some(6000), &[], &[], 0);
    let start_snapshot = wait_for_action(Duration::from_secs(5));
    assert_eq!(start_snapshot.overlay_message.as_deref(), Some("Fabric START sees Tester_2 at 6000 game 300 build -64..320 rain true/0.5 thunder false/0.2"));
    assert!(!start_snapshot.animate);
    fabric_bridge::dispatch_end_tick(snapshot.clone(), Some(6001), &[], 0);
    let end_snapshot = wait_for_action(Duration::from_secs(5));
    assert_eq!(
        end_snapshot.overlay_message.as_deref(),
        Some("Fabric player Tester_2 xyz 1.25,64.0,-3.5 block 1,64,-4 state minecraft:stone air false solid true v 0.3,0.4,0.0 yaw 37.5 pitch -12.5 hp 18.5 sprint true ground false day 6001 game 300 height -64..320 rain true/0.5 thunder false/0.2")
    );
    assert!(!end_snapshot.animate);
    assert_eq!(
        end_snapshot.sprinting,
        Some(true),
        "the end-tick sprint-key pattern did not reach Horizon's native action"
    );
    assert!(fabric_bridge::dispatch_start_tick(snapshot.clone(), Some(6002), &[19], &[19], 0));
    let key_held = wait_for_action(Duration::from_secs(5));
    assert_eq!(key_held.overlay_message.as_deref(), Some("Fabric key held P"));
    assert!(!key_held.animate);
    assert!(fabric_bridge::dispatch_end_tick(
        snapshot,
        Some(6003),
        &[19],
        fabric_bridge::VANILLA_INPUT_UP,
    ));
    let key_click = wait_for_action(Duration::from_secs(5));
    assert_eq!(key_click.overlay_message.as_deref(), Some("Fabric forward=true sneak=false forwardPressed=true; sprint enabled; look direction and vertical velocity queued"));
    assert_eq!(key_click.system_message.as_deref(), Some("Fabric system chat message mapped into Horizon"));
    assert!(!key_click.animate);
    assert_eq!(key_click.sprinting, Some(true));
    assert_eq!(key_click.player_velocity, Some([0.0, 0.5, 0.0]));
    assert_eq!(key_click.player_rotation, Some([127.5, 15.0]));
    assert_eq!(fabric_bridge::dispatched_callbacks(), 6);

    shutdown_stream
        .write_all(b"stop\n")
        .expect("could not stop Java helper cleanly");
    wait_for_helper(&mut helper.0);
    println!(concat!(
        "Rust-to-Java loopback passed: unchanged probe JAR -> ordered Fabric START/END_CLIENT_TICK -> ",
        "player/world/environment and bounded nearby-block snapshots, semantic movement input, keyboard input, ",
        "and queued sprint/velocity/look/system-chat actions -> Horizon hooks."
    ));
}

fn prepare_real_loader_mod(arguments: &[String]) {
    let root = Path::new(&arguments[4]);
    let mods = root.join("mods");
    let game = root.join("game");
    std::fs::create_dir_all(&mods).expect("could not create the temporary Fabric mods folder");
    std::fs::create_dir_all(&game).expect("could not create the temporary Fabric game folder");
    std::fs::copy(&arguments[3], mods.join("opnlauncher-fabric-bridge-probe.jar"))
        .expect("could not stage the temporary Fabric bridge probe");
}

fn spawn_helper(arguments: &[String], port: u16) -> Child {
    let mut command = Command::new(&arguments[0]);
    command.arg(format!("-Dopn.fabric.bridge.port={port}"));

    let (classpath, entrypoint, game_dir) = if arguments.len() == 4 {
        command.arg(format!("-Dopn.fabric.test.probe.jar={}", arguments[2]));
        (
            arguments[1].clone(),
            "org.opnlauncher.horizonfabric.HorizonFabricBridgeSocketTest",
            arguments[3].clone(),
        )
    } else {
        command
            .arg("-Dfabric.skipMcProvider=true")
            .arg("-Dfabric.side=client")
            .arg("-Dfabric.noGui=true")
            .arg("-Dfabric.gameVersion=26.3")
            .arg(format!("-Dfabric.modsFolder={}/mods", arguments[4]))
            .arg(format!("-Dopn.fabric.gameDir={}/game", arguments[4]));
        let mut libraries = Vec::new();
        collect_jars(Path::new(&arguments[2]), &mut libraries);
        libraries.sort();
        let mut classpath_entries = vec![PathBuf::from(&arguments[1])];
        classpath_entries.extend(libraries);
        let classpath = std::env::join_paths(classpath_entries)
            .expect("could not construct the Fabric Loader class path")
            .to_string_lossy()
            .into_owned();
        (
            classpath,
            "net.fabricmc.loader.impl.launch.knot.KnotClient",
            arguments[4].clone(),
        )
    };

    command
        .arg("-cp")
        .arg(classpath)
        .arg(entrypoint)
        .current_dir(game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("could not start the Java Fabric bridge process")
}

fn collect_jars(directory: &Path, output: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(directory)
        .expect("the configured Fabric Loader libraries folder could not be read");
    for entry in entries {
        let path = entry.expect("a Fabric Loader library entry could not be read").path();
        if path.is_dir() {
            collect_jars(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "jar") {
            output.push(path);
        }
    }
}

struct JavaHelper(Child);

impl Drop for JavaHelper {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn wait_for_action(timeout: Duration) -> fabric_bridge::FabricBridgeAction {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(action) = fabric_bridge::take_actions().into_iter().next() {
            return action;
        }
        assert!(Instant::now() < deadline, "timed out waiting for the Java action-bar response");
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_helper(helper: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = helper.try_wait().expect("could not inspect Java helper") {
            assert!(status.success(), "Java helper exited unsuccessfully: {status}");
            return;
        }
        if Instant::now() >= deadline {
            let _ = helper.kill();
            let _ = helper.wait();
            panic!("Java helper did not stop after the bridge stop message");
        }
        thread::sleep(Duration::from_millis(10));
    }
}
