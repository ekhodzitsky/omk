use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::process;
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 1 {
        eprintln!(
            "Usage: mock-kimi [--version | --help | --wire | -p <prompt_file>] [--stall] [--slow] [--malformed] [--crash-after-turn-begin]"
        );
        process::exit(1);
    }

    let stall = args.iter().any(|a| a == "--stall");
    let slow = args.iter().any(|a| a == "--slow");
    let malformed = args.iter().any(|a| a == "--malformed");
    let crash_after_turn_begin = args.iter().any(|a| a == "--crash-after-turn-begin");

    match args[1].as_str() {
        "--version" => {
            println!("kimi version 0.1.0-mock");
            process::exit(0);
        }
        "--help" => {
            println!("mock-kimi -- a stub for the Kimi CLI");
            println!();
            println!("Usage:");
            println!("  mock-kimi --version          Print version");
            println!("  mock-kimi --help             Print this help");
            println!("  mock-kimi --wire             Run in wire protocol mode");
            println!("  mock-kimi -p <prompt_file>   Run with a prompt file");
            println!("  mock-kimi --stall            Enable stall mode");
            println!("  mock-kimi --slow             Enable slow streaming mode");
            println!("  mock-kimi --malformed        Emit malformed output");
            println!("  mock-kimi --crash-after-turn-begin");
            println!("                               Crash immediately after turn_begin");
            process::exit(0);
        }
        "--wire" => run_wire_mode(stall, slow, malformed, crash_after_turn_begin),
        "-p" => run_prompt_mode(&args, stall, slow, malformed),
        _ => {
            eprintln!("Unknown argument: {}", args[1]);
            process::exit(1);
        }
    }
}

fn run_prompt_mode(args: &[String], stall: bool, slow: bool, malformed: bool) {
    if args.len() < 3 {
        eprintln!("Error: -p requires a prompt file argument");
        process::exit(1);
    }
    let prompt_file = &args[2];
    let prompt = match fs::read_to_string(prompt_file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "{{\"status\":\"error\",\"message\":\"Failed to read prompt file: {}\"}}",
                e
            );
            process::exit(1);
        }
    };

    // Simulate processing time
    let delay = if slow {
        Duration::from_secs(1)
    } else {
        Duration::from_millis(200)
    };
    thread::sleep(delay);

    let preview: String = prompt.chars().take(80).collect();

    if malformed {
        println!("{{ this is not valid json");
        process::exit(0);
    }

    let lower = prompt.to_lowercase();
    if lower.contains("error") || lower.contains("fail") {
        eprintln!("{{\"status\":\"error\",\"mock\":true,\"message\":\"Mock error triggered\"}}");
        process::exit(1);
    }

    let is_stall = stall || lower.contains("stall");

    if is_stall {
        let mut stdout = io::stdout();
        writeln!(
            stdout,
            "{{\"status\":\"partial\",\"mock\":true,\"message\":\"Stalling...\"}}"
        )
        .ok();
        stdout.flush().ok();

        let heartbeat_interval = if slow {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(5)
        };
        loop {
            thread::sleep(heartbeat_interval);
            writeln!(
                stdout,
                "{{\"status\":\"heartbeat\",\"mock\":true,\"message\":\"still alive\"}}"
            )
            .ok();
            stdout.flush().ok();
        }
    }

    let response = if lower.contains("test") {
        "I see you want to run tests."
    } else {
        &format!("Mock Kimi response: {}", preview)
    };

    println!(
        "{{\"status\":\"success\",\"mock\":true,\"prompt_preview\":\"{}\",\"response\":\"{}\"}}",
        preview.replace('"', "\\\""),
        response.replace('"', "\\\"")
    );
    process::exit(0);
}

fn run_wire_mode(stall: bool, slow: bool, malformed: bool, crash_after_turn_begin: bool) {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }

        // Parse minimal JSON-RPC to route
        let Ok(req): Result<serde_json::Value, _> = serde_json::from_str(&line) else {
            continue;
        };

        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = req.get("id").and_then(|i| i.as_str()).unwrap_or("");

        if malformed {
            writeln!(stdout, "{{ this is not valid json").ok();
            stdout.flush().ok();
            break;
        }

        match method {
            "initialize" => {
                let resp = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                        "protocol_version": "1.9",
                        "server": {"name": "mock-kimi", "version": "0.1.0"}
                    }
                });
                writeln!(stdout, "{}", resp).ok();
                stdout.flush().ok();
            }
            "prompt" => {
                let user_input = req
                    .get("params")
                    .and_then(|p| p.get("user_input"))
                    .and_then(|u| u.as_str())
                    .unwrap_or("mock task");
                let preview: String = user_input.chars().take(60).collect();
                let lower = user_input.to_lowercase();
                let is_stall = stall || lower.contains("stall");
                let is_crash_after_turn_begin =
                    crash_after_turn_begin || lower.contains("crash-after-turn-begin");

                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {"status": "ok", "steps": [{"n": 1}]}
                });
                writeln!(stdout, "{}", resp).ok();
                stdout.flush().ok();

                let event_delay = if slow {
                    Duration::from_secs(1)
                } else {
                    Duration::from_millis(100)
                };
                thread::sleep(event_delay);
                emit_event(
                    &mut stdout,
                    "turn_begin",
                    serde_json::json!({"user_input": preview}),
                );

                if is_crash_after_turn_begin {
                    process::abort();
                }

                if is_stall {
                    let heartbeat_interval = if slow {
                        Duration::from_secs(1)
                    } else {
                        Duration::from_secs(5)
                    };
                    loop {
                        thread::sleep(heartbeat_interval);
                        emit_event(
                            &mut stdout,
                            "heartbeat",
                            serde_json::json!({"status": "stalling"}),
                        );
                    }
                }

                thread::sleep(event_delay);
                maybe_write_mock_project_file();

                let text = format!("Mock wire response for: {}", preview);
                emit_event(
                    &mut stdout,
                    "content_part",
                    serde_json::json!({"text": text}),
                );

                thread::sleep(if slow {
                    Duration::from_secs(1)
                } else {
                    Duration::from_millis(50)
                });
                emit_event(&mut stdout, "turn_end", serde_json::json!({}));
            }
            "cancel" => {
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {}
                });
                writeln!(stdout, "{}", resp).ok();
                stdout.flush().ok();
            }
            "replay" => {
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "status": "finished",
                        "events": [],
                        "requests": []
                    }
                });
                writeln!(stdout, "{}", resp).ok();
                stdout.flush().ok();
            }
            "steer" => {
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {"status": "steered"}
                });
                writeln!(stdout, "{}", resp).ok();
                stdout.flush().ok();
            }
            "set_plan_mode" => {
                let enabled = req
                    .get("params")
                    .and_then(|p| p.get("enabled"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {"status": "ok", "plan_mode": enabled}
                });
                writeln!(stdout, "{}", resp).ok();
                stdout.flush().ok();
            }
            _ => {
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": -32601, "message": "Method not found"}
                });
                writeln!(stdout, "{}", resp).ok();
                stdout.flush().ok();
            }
        }
    }
}

fn maybe_write_mock_project_file() {
    let Ok(path) = env::var("MOCK_KIMI_WRITE_FILE") else {
        return;
    };
    if path.trim().is_empty() || path.contains("..") || path.starts_with('/') {
        return;
    }

    let body = env::var("MOCK_KIMI_WRITE_BODY")
        .unwrap_or_else(|_| "mock kimi project mutation\n".to_string());
    if let Some(parent) = std::path::Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    let _ = fs::write(path, body);
}

fn emit_event(stdout: &mut io::Stdout, event_type: &str, payload: serde_json::Value) {
    let event = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "event",
        "params": {
            "type": event_type,
            "payload": payload
        }
    });
    writeln!(stdout, "{}", event).ok();
    stdout.flush().ok();
}
