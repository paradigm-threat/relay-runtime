//! Virtual `@clevertree/video` host module: cross-platform **`VideoPlayer`** native component.
//!
//! Returns a `relay-video-player` VNode consumed by web / Android / desktop glue.
//! CleverScript controls playback via props (`playing`, `currentTime`, `command`/`commandSeq`)
//! and handlers (`onPlay`, `onPause`, `onTimeUpdate`, …). Hosts should throttle `onTimeUpdate`.

use crate::value::Value;
use serde_json;
use std::collections::HashMap;

pub const VIDEO_PLAYER_TAG: &str = "relay-video-player";

/// Standard props documented for CleverScript authors.
pub const VIDEO_PROP_SRC: &str = "src";
pub const VIDEO_PROP_POSTER: &str = "poster";
pub const VIDEO_PROP_CONTROLS: &str = "controls";
pub const VIDEO_PROP_AUTO_PLAY: &str = "autoPlay";
pub const VIDEO_PROP_PLAYING: &str = "playing";
pub const VIDEO_PROP_MUTED: &str = "muted";
pub const VIDEO_PROP_LOOP: &str = "loop";
pub const VIDEO_PROP_CURRENT_TIME: &str = "currentTime";
pub const VIDEO_PROP_VOLUME: &str = "volume";
pub const VIDEO_PROP_PLAYBACK_RATE: &str = "playbackRate";
pub const VIDEO_PROP_SUBTITLES_JSON: &str = "subtitlesJson";
pub const VIDEO_PROP_COMMAND: &str = "command";
pub const VIDEO_PROP_COMMAND_SEQ: &str = "commandSeq";
pub const VIDEO_PROP_COMMAND_ARG: &str = "commandArg";
pub const VIDEO_PROP_PLAYER_ID: &str = "playerId";

pub fn video_module() -> Value {
    let mut m = HashMap::new();
    m.insert(
        "VideoPlayer".to_string(),
        Value::Native(video_player_native),
    );
    Value::Object(m)
}

fn element_value(tag: &str, props: HashMap<String, Value>, children: Vec<Value>) -> Value {
    let mut obj = HashMap::new();
    obj.insert("type".to_string(), Value::String("element".to_string()));
    obj.insert("tag".to_string(), Value::String(tag.to_string()));
    obj.insert("props".to_string(), Value::Object(props));
    obj.insert("children".to_string(), Value::Array(children));
    Value::Object(obj)
}

fn copy_string_prop(out: &mut HashMap<String, Value>, src: &HashMap<String, Value>, key: &str) {
    if let Some(Value::String(s)) = src.get(key) {
        if !s.is_empty() {
            out.insert(key.to_string(), Value::String(s.clone()));
        }
    }
}

fn copy_bool_prop(out: &mut HashMap<String, Value>, src: &HashMap<String, Value>, key: &str) {
    if let Some(Value::Bool(b)) = src.get(key) {
        out.insert(key.to_string(), Value::Bool(*b));
    }
}

fn copy_number_prop(out: &mut HashMap<String, Value>, src: &HashMap<String, Value>, key: &str) {
    if let Some(Value::Number(n)) = src.get(key) {
        out.insert(key.to_string(), Value::Number(*n));
    }
}

fn copy_handler_prop(out: &mut HashMap<String, Value>, src: &HashMap<String, Value>, key: &str) {
    if let Some(v) = src.get(key) {
        match v {
            Value::Function { .. } | Value::Native(_) | Value::NativeFn(_) | Value::Setter(_) => {
                out.insert(key.to_string(), v.clone());
            }
            _ => {}
        }
    }
}

fn subtitles_to_json(props: &HashMap<String, Value>) -> Option<String> {
    let tracks = props.get("subtitles")?;
    let json_val = tracks.to_json();
    serde_json::to_string(&json_val).ok()
}

/// Build `relay-video-player` element props from CleverScript `VideoPlayer` props.
pub fn video_player_props_from_map(props_map: &HashMap<String, Value>) -> HashMap<String, Value> {
    let mut out = HashMap::new();
    for key in [
        VIDEO_PROP_SRC,
        VIDEO_PROP_POSTER,
        "className",
        VIDEO_PROP_PLAYER_ID,
    ] {
        copy_string_prop(&mut out, props_map, key);
    }
    for key in [
        VIDEO_PROP_CONTROLS,
        VIDEO_PROP_AUTO_PLAY,
        VIDEO_PROP_PLAYING,
        VIDEO_PROP_MUTED,
        VIDEO_PROP_LOOP,
    ] {
        copy_bool_prop(&mut out, props_map, key);
    }
    for key in [
        VIDEO_PROP_CURRENT_TIME,
        VIDEO_PROP_VOLUME,
        VIDEO_PROP_PLAYBACK_RATE,
        VIDEO_PROP_COMMAND,
        VIDEO_PROP_COMMAND_SEQ,
        VIDEO_PROP_COMMAND_ARG,
    ] {
        copy_number_prop(&mut out, props_map, key);
    }
    for key in [
        "onPlay",
        "onPause",
        "onEnded",
        "onTimeUpdate",
        "onError",
        "onLoadedMetadata",
        "onSeeked",
    ] {
        copy_handler_prop(&mut out, props_map, key);
    }
    if let Some(json) = subtitles_to_json(props_map) {
        out.insert(VIDEO_PROP_SUBTITLES_JSON.to_string(), Value::String(json));
    }
    // Default: show native controls unless explicitly false.
    if !out.contains_key(VIDEO_PROP_CONTROLS) {
        out.insert(VIDEO_PROP_CONTROLS.to_string(), Value::Bool(true));
    }
    out
}

fn video_player_native(args: &[Value]) -> Result<Value, String> {
    let props = args
        .first()
        .ok_or_else(|| "VideoPlayer: missing props".to_string())?;
    let Value::Object(props_map) = props else {
        return Err("VideoPlayer: props must be an object".to_string());
    };
    if props_map
        .get(VIDEO_PROP_SRC)
        .and_then(|v| {
            if let Value::String(s) = v {
                Some(s.is_empty())
            } else {
                None
            }
        })
        .unwrap_or(true)
    {
        return Err("VideoPlayer: src is required".to_string());
    }
    let out_props = video_player_props_from_map(props_map);
    Ok(element_value(VIDEO_PLAYER_TAG, out_props, vec![]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vdom::{value_to_vnode_with_handlers, PropValue, VNode};

    #[test]
    fn video_player_emits_relay_tag_and_subtitles_json() {
        let mut props = HashMap::new();
        props.insert(
            "src".to_string(),
            Value::String("/media/foo.mkv".to_string()),
        );
        props.insert("controls".to_string(), Value::Bool(true));
        props.insert(
            "subtitles".to_string(),
            Value::Array(vec![Value::Object({
                let mut m = HashMap::new();
                m.insert("src".to_string(), Value::String("/subs.en.vtt".to_string()));
                m.insert("label".to_string(), Value::String("English".to_string()));
                m.insert("lang".to_string(), Value::String("en".to_string()));
                m.insert("default".to_string(), Value::Bool(true));
                m
            })]),
        );
        let v = video_player_native(&[Value::Object(props)]).unwrap();
        let vn = value_to_vnode_with_handlers(&v, &mut |_| 0);
        match vn {
            VNode::Element { tag, props, .. } => {
                assert_eq!(tag, VIDEO_PLAYER_TAG);
                assert_eq!(
                    props.get("src"),
                    Some(&PropValue::String("/media/foo.mkv".to_string()))
                );
                assert!(props.contains_key("subtitlesJson"));
            }
            _ => panic!("expected element"),
        }
    }
}
