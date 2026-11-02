//! Status line and chat log. DOM on wasm, stdout on native.

use bevy::prelude::*;
use voxel_common::block;

use crate::Game;

pub fn set(id: &str, text: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        if let Some(el) = doc.get_element_by_id(id) {
            el.set_text_content(Some(text));
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    println!("[{id}] {text}");
}

pub fn log(line: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let Some(el) = doc.get_element_by_id("log") else {
            return;
        };
        let Ok(div) = doc.create_element("div") else {
            return;
        };
        div.set_text_content(Some(line));
        let _ = el.append_child(&div);
        while el.child_nodes().length() > 8 {
            let Some(first) = el.child_nodes().item(0) else {
                break;
            };
            let _ = el.remove_child(&first);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    println!("{line}");
}

pub fn input_focused() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.active_element())
            .map(|e| e.tag_name().to_lowercase() == "input")
            .unwrap_or(false)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}

pub fn update(mut game: ResMut<Game>) {
    if !game.dirty_hud {
        return;
    }
    game.dirty_hud = false;

    let mut status = format!(
        "{} | {} online | x{:0.2} y{:0.2} z{:0.2} yaw{:0.2} pitch{:0.2}",
        game.name,
        game.roster.len(),
        game.local.feet.x,
        game.local.feet.y,
        game.local.feet.z,
        game.local.yaw,
        game.local.pitch
    );
    if game.aim_visible {
        status.push_str(&format!(
            " aim {:0.2} {:0.2} {:0.2}",
            game.aim_point.x, game.aim_point.y, game.aim_point.z
        ));
    }
    if status != game.last_status {
        game.last_status = status.clone();
        set("status", &status);
    }

    let mut names: Vec<(u64, String)> =
        game.roster.iter().map(|(id, n)| (*id, n.clone())).collect();
    names.sort();
    let roster = names
        .iter()
        .map(|(id, n)| {
            if *id == game.my_id {
                format!("{n} (you)")
            } else {
                n.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    if roster != game.last_roster {
        game.last_roster = roster.clone();
        set("players", &roster);
    }

    let hotbar = block::HOTBAR
        .iter()
        .enumerate()
        .map(|(i, b)| {
            if i as u8 == game.local.selected {
                format!("[{i} {}]", block::name(*b))
            } else {
                format!("{i} {}", block::name(*b))
            }
        })
        .collect::<Vec<_>>()
        .join("  ");
    if hotbar != game.last_hotbar {
        game.last_hotbar = hotbar.clone();
        set("hotbar", &hotbar);
    }
}
