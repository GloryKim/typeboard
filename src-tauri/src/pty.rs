use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tauri::ipc::Channel;
use tauri::State;

pub struct PtySession {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    child_killer: Mutex<Box<dyn portable_pty::ChildKiller + Send + Sync>>,
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if let Ok(mut killer) = self.child_killer.lock() {
            let _ = killer.kill();
        }
    }
}

#[derive(Default)]
pub struct PtyState {
    next_id: AtomicU32,
    sessions: Mutex<HashMap<u32, Arc<PtySession>>>,
}

impl PtyState {
    fn insert(&self, id: u32, session: Arc<PtySession>) {
        self.sessions.lock().expect("pty map").insert(id, session);
    }

    fn get(&self, id: u32) -> Result<Arc<PtySession>, String> {
        self.sessions
            .lock()
            .expect("pty map")
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("unknown pty session {id}"))
    }

    fn take(&self, id: u32) -> Option<Arc<PtySession>> {
        self.sessions.lock().expect("pty map").remove(&id)
    }
}

fn default_cwd() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| {
        if cfg!(windows) {
            "powershell.exe".into()
        } else {
            "/bin/zsh".into()
        }
    })
}

#[tauri::command]
pub fn pty_spawn(
    state: State<PtyState>,
    cols: u16,
    rows: u16,
    cwd: Option<String>,
    on_data: Channel<Vec<u8>>,
    on_exit: Channel<i32>,
) -> Result<u32, String> {
    let cols = cols.max(2);
    let rows = rows.max(1);
    let id = state.next_id.fetch_add(1, Ordering::Relaxed);

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("open pty: {e}"))?;

    let mut cmd = CommandBuilder::new(default_shell());
    #[cfg(unix)]
    {
        cmd.arg("-l");
    }
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("TERM_PROGRAM", "typeboard");
    cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));

    let cwd = cwd
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_cwd);
    cmd.cwd(cwd);

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("spawn shell: {e}"))?;
    let killer = child.clone_killer();

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("clone pty reader: {e}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("take pty writer: {e}"))?;

    let session = Arc::new(PtySession {
        master: Mutex::new(pair.master),
        writer: Mutex::new(writer),
        child_killer: Mutex::new(killer),
    });
    state.insert(id, session);
    eprintln!(
        "pty {id} spawned cols={cols} rows={rows} shell={}",
        default_shell()
    );

    thread::Builder::new()
        .name(format!("pty-read-{id}"))
        .spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if on_data.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        })
        .map_err(|e| format!("spawn reader thread: {e}"))?;

    thread::Builder::new()
        .name(format!("pty-wait-{id}"))
        .spawn(move || {
            let status = loop {
                match child.try_wait() {
                    Ok(Some(status)) => break status,
                    Ok(None) => thread::sleep(Duration::from_millis(40)),
                    Err(_) => {
                        let _ = on_exit.send(1);
                        return;
                    }
                }
            };
            let code = i32::try_from(status.exit_code()).unwrap_or(1);
            let _ = on_exit.send(code);
        })
        .map_err(|e| format!("spawn wait thread: {e}"))?;

    Ok(id)
}

#[tauri::command]
pub fn pty_write(state: State<PtyState>, id: u32, data: String) -> Result<(), String> {
    let session = state.get(id)?;
    let mut writer = session.writer.lock().map_err(|e| e.to_string())?;
    writer
        .write_all(data.as_bytes())
        .map_err(|e| format!("pty write: {e}"))?;
    writer.flush().map_err(|e| format!("pty flush: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn pty_resize(state: State<PtyState>, id: u32, cols: u16, rows: u16) -> Result<(), String> {
    let session = state.get(id)?;
    let master = session.master.lock().map_err(|e| e.to_string())?;
    master
        .resize(PtySize {
            rows: rows.max(1),
            cols: cols.max(2),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("pty resize: {e}"))
}

#[tauri::command]
pub fn pty_close(state: State<PtyState>, id: u32) -> Result<(), String> {
    if let Some(session) = state.take(id) {
        if let Ok(mut killer) = session.child_killer.lock() {
            let _ = killer.kill();
        }
    }
    Ok(())
}

#[tauri::command]
pub fn pty_tab_title(state: State<PtyState>, id: u32) -> Result<Option<String>, String> {
    let session = state.get(id)?;
    let master = session.master.lock().map_err(|e| e.to_string())?;
    Ok(foreground_label(&**master))
}

fn foreground_label(master: &dyn MasterPty) -> Option<String> {
    #[cfg(unix)]
    {
        let pgid = master.process_group_leader()?;
        pick_label(&process_group_names(pgid))
    }
    #[cfg(not(unix))]
    {
        let _ = master;
        None
    }
}

#[cfg(unix)]
fn pick_label(names: &[String]) -> Option<String> {
    const SHELLS: &[&str] = &[
        "zsh",
        "bash",
        "fish",
        "sh",
        "dash",
        "ksh",
        "csh",
        "tcsh",
        "pwsh",
        "powershell",
        "login",
    ];
    const WRAPPERS: &[&str] = &[
        "sudo", "doas", "env", "nice", "nohup", "time", "stdbuf", "unbuffer", "script",
    ];

    let cleaned: Vec<String> = names
        .iter()
        .map(|n| n.trim().trim_start_matches('-').to_string())
        .filter(|n| !n.is_empty())
        .collect();

    cleaned
        .iter()
        .find(|n| !SHELLS.contains(&n.as_str()) && !WRAPPERS.contains(&n.as_str()))
        .cloned()
        .or_else(|| {
            cleaned
                .iter()
                .find(|n| !SHELLS.contains(&n.as_str()))
                .cloned()
        })
        .or_else(|| cleaned.last().cloned())
}

#[cfg(target_os = "macos")]
fn process_name(pid: libc::pid_t) -> Option<String> {
    let mut buf = [0u8; 64];
    let n = unsafe {
        libc::proc_name(
            pid,
            buf.as_mut_ptr() as *mut libc::c_void,
            buf.len() as u32,
        )
    };
    if n <= 0 {
        return None;
    }
    let n = (n as usize).min(buf.len());
    let s = String::from_utf8_lossy(&buf[..n])
        .trim_end_matches('\0')
        .trim()
        .to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[cfg(target_os = "macos")]
fn process_group_names(pgid: libc::pid_t) -> Vec<String> {
    const PROC_PGRP_ONLY: u32 = 2;
    let needed =
        unsafe { libc::proc_listpids(PROC_PGRP_ONLY, pgid as u32, std::ptr::null_mut(), 0) };
    if needed <= 0 {
        return process_name(pgid).into_iter().collect();
    }
    let cap = (needed as usize / std::mem::size_of::<libc::pid_t>()).max(8);
    let mut pids = vec![0 as libc::pid_t; cap];
    let got = unsafe {
        libc::proc_listpids(
            PROC_PGRP_ONLY,
            pgid as u32,
            pids.as_mut_ptr() as *mut libc::c_void,
            (pids.len() * std::mem::size_of::<libc::pid_t>()) as i32,
        )
    };
    if got <= 0 {
        return process_name(pgid).into_iter().collect();
    }
    let n = got as usize / std::mem::size_of::<libc::pid_t>();
    let names: Vec<String> = pids
        .into_iter()
        .take(n)
        .filter(|p| *p > 0)
        .filter_map(process_name)
        .collect();
    if names.is_empty() {
        process_name(pgid).into_iter().collect()
    } else {
        names
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn process_name(pid: libc::pid_t) -> Option<String> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let n = s.trim();
    if n.is_empty() {
        None
    } else {
        Some(n.to_string())
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn parse_pgrp(stat: &str) -> Option<libc::pid_t> {
    let close = stat.rfind(')')?;
    let rest = stat.get(close + 1..)?;
    rest.split_whitespace().nth(2)?.parse().ok()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn process_group_names(pgid: libc::pid_t) -> Vec<String> {
    let mut names = Vec::new();
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return process_name(pgid).into_iter().collect();
    };
    for ent in rd.flatten() {
        let pid: libc::pid_t = match ent.file_name().to_str().and_then(|s| s.parse().ok()) {
            Some(p) => p,
            None => continue,
        };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        if parse_pgrp(&stat) != Some(pgid) {
            continue;
        }
        if let Some(n) = process_name(pid) {
            names.push(n);
        }
    }
    if names.is_empty() {
        names.extend(process_name(pgid));
    }
    names
}
