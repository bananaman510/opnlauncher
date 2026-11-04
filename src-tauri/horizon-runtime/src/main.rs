//! OpnLauncher builds Horizon's upstream native client as its managed game process.
//! Keep the allocator and mod API marker used by Horizon's own executable.

#[cfg(not(feature = "system-allocator"))]
#[global_allocator]
static ALLOCATOR: horizon_threads::Allocator = horizon_threads::Allocator;

static MOD_API_LEVEL: [u8; 27] = horizon_mods::api_level::MARK;

fn main() {
    std::hint::black_box(&MOD_API_LEVEL);
    let mut fabric_bridge = start_fabric_bridge_from_environment();
    horizon_client::entry::main();
    if let Some(child) = &mut fabric_bridge {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn start_fabric_bridge_from_environment() -> Option<std::process::Child> {
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    const PREFIX: &str = "OPNLAUNCHER_FABRIC_BRIDGE_";
    let value = |name: &str| std::env::var(format!("{PREFIX}{name}")).ok();
    let (
        Some(java),
        Some(class_path),
        Some(helper_jar),
        Some(mods_dir),
        Some(game_dir),
    ) = (
        value("JAVA"),
        value("CLASS_PATH"),
        value("JAR"),
        value("MODS_DIR"),
        value("GAME_DIR"),
    ) else {
        return None;
    };
    if !std::path::Path::new(&helper_jar).is_file() {
        eprintln!("Horizon Fabric bridge: the bundled helper JAR is missing; continuing with native Horizon only.");
        return None;
    }

    let listener = match TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("Horizon Fabric bridge: could not create its local event socket: {error}");
            return None;
        }
    };
    if let Err(error) = listener.set_nonblocking(true) {
        eprintln!("Horizon Fabric bridge: could not configure its local event socket: {error}");
        return None;
    }
    let port = match listener.local_addr() {
        Ok(address) => address.port(),
        Err(error) => {
            eprintln!("Horizon Fabric bridge: could not read its local event socket: {error}");
            return None;
        }
    };
    let separator = if cfg!(windows) { ";" } else { ":" };
    let combined_class_path = format!("{helper_jar}{separator}{class_path}");
    let mut command = Command::new(java);
    command
        .arg("-Xmx1024m")
        .arg("-Dfabric.skipMcProvider=true")
        .arg("-Dfabric.side=client")
        .arg("-Dfabric.noGui=true")
        .arg("-Dfabric.gameVersion=26.3")
        .arg(format!("-Dfabric.modsFolder={mods_dir}"))
        .arg(format!("-Dopn.fabric.gameDir={game_dir}"))
        .arg(format!("-Dopn.fabric.bridge.port={port}"));
    if let Ok(count) = value("JVM_ARG_COUNT").unwrap_or_default().parse::<usize>() {
        for index in 0..count.min(64) {
            if let Some(argument) = value(&format!("JVM_ARG_{index}")) {
                if !argument.starts_with("-cp") && !argument.starts_with("-classpath") {
                    command.arg(argument);
                }
            }
        }
    }
    let mut child = match command
        .arg("-cp")
        .arg(combined_class_path)
        .arg("net.fabricmc.loader.impl.launch.knot.KnotClient")
        .current_dir(game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            eprintln!("Horizon Fabric bridge: could not start the Java sidecar: {error}; continuing with native Horizon only.");
            return None;
        }
    };

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let mut handshake_reader = match stream.try_clone() {
                    Ok(clone) => BufReader::new(clone),
                    Err(error) => {
                        eprintln!("Horizon Fabric bridge: could not prepare the local event connection: {error}");
                        let _ = child.kill();
                        let _ = child.wait();
                        return None;
                    }
                };
                let mut handshake = String::new();
                match handshake_reader.read_line(&mut handshake) {
                    Ok(_) if handshake.trim() == "ready" => {
                        if let Err(error) = horizon_client::fabric_bridge::attach(stream) {
                            eprintln!("Horizon Fabric bridge: could not attach the tick dispatcher: {error}");
                            let _ = child.kill();
                            let _ = child.wait();
                            return None;
                        }
                        eprintln!("Horizon Fabric bridge: Fabric Loader sidecar connected; forwarding Horizon client ticks to END_CLIENT_TICK.");
                        return Some(child);
                    }
                    Ok(_) => {
                        eprintln!("Horizon Fabric bridge: Java sidecar did not complete its startup handshake.");
                        let _ = child.kill();
                        let _ = child.wait();
                        return None;
                    }
                    Err(error) => {
                        eprintln!("Horizon Fabric bridge: startup handshake failed: {error}");
                        let _ = child.kill();
                        let _ = child.wait();
                        return None;
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    eprintln!("Horizon Fabric bridge: Java sidecar did not connect in time; continuing with native Horizon only.");
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                if let Ok(Some(status)) = child.try_wait() {
                    eprintln!("Horizon Fabric bridge: Java sidecar exited during startup ({status}); continuing with native Horizon only.");
                    return None;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(error) => {
                eprintln!("Horizon Fabric bridge: local event socket failed: {error}; continuing with native Horizon only.");
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}
