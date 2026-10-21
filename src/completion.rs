/// Metadata used for interactive command completion and help.
pub struct CommandSpec {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub usage: &'static str,
    pub description: &'static str,
}

static COMMAND_SPECS: &[CommandSpec] = &[
    CommandSpec {
        name: "quit",
        aliases: &["q", "x"],
        usage: "quit",
        description: "Quit ncspot",
    },
    CommandSpec {
        name: "playpause",
        aliases: &["pause", "toggleplay", "toggleplayback"],
        usage: "playpause",
        description: "Toggle playback",
    },
    CommandSpec {
        name: "stop",
        aliases: &[],
        usage: "stop",
        description: "Stop playback",
    },
    CommandSpec {
        name: "previous",
        aliases: &[],
        usage: "previous",
        description: "Play the previous track",
    },
    CommandSpec {
        name: "next",
        aliases: &[],
        usage: "next",
        description: "Play the next track",
    },
    CommandSpec {
        name: "clear",
        aliases: &[],
        usage: "clear",
        description: "Clear the queue",
    },
    CommandSpec {
        name: "queue",
        aliases: &[],
        usage: "queue",
        description: "Add the selected item to the queue",
    },
    CommandSpec {
        name: "playnext",
        aliases: &[],
        usage: "playnext",
        description: "Play the selected item next",
    },
    CommandSpec {
        name: "play",
        aliases: &[],
        usage: "play",
        description: "Play the selected item",
    },
    CommandSpec {
        name: "update",
        aliases: &[],
        usage: "update",
        description: "Refresh the library from Spotify",
    },
    CommandSpec {
        name: "save",
        aliases: &[],
        usage: "save [current|queue]",
        description: "Save the selected item, the current track, or the queue",
    },
    CommandSpec {
        name: "add",
        aliases: &[],
        usage: "add [current]",
        description: "Add the selected or current track to a playlist",
    },
    CommandSpec {
        name: "delete",
        aliases: &[],
        usage: "delete",
        description: "Delete the selected item",
    },
    CommandSpec {
        name: "focus",
        aliases: &[],
        usage: "focus <queue|search|library|cover>",
        description: "Switch to another screen",
    },
    CommandSpec {
        name: "seek",
        aliases: &[],
        usage: "seek [+|-]<time>",
        description: "Seek to a position, or relative with + or -",
    },
    CommandSpec {
        name: "volup",
        aliases: &[],
        usage: "volup [amount]",
        description: "Increase volume by amount percent",
    },
    CommandSpec {
        name: "voldown",
        aliases: &[],
        usage: "voldown [amount]",
        description: "Decrease volume by amount percent",
    },
    CommandSpec {
        name: "repeat",
        aliases: &["loop"],
        usage: "repeat [list|track|none]",
        description: "Set or cycle the repeat mode",
    },
    CommandSpec {
        name: "shuffle",
        aliases: &[],
        usage: "shuffle [on|off]",
        description: "Enable, disable, or toggle shuffle",
    },
    CommandSpec {
        name: "share",
        aliases: &[],
        usage: "share <selected|current>",
        description: "Copy a Spotify link to the clipboard",
    },
    CommandSpec {
        name: "back",
        aliases: &[],
        usage: "back",
        description: "Go back to the previous view",
    },
    CommandSpec {
        name: "open",
        aliases: &[],
        usage: "open <selected|current>",
        description: "Open the selected or current item",
    },
    CommandSpec {
        name: "goto",
        aliases: &[],
        usage: "goto <album|artist>",
        description: "Open the album or artist of the selected item",
    },
    CommandSpec {
        name: "move",
        aliases: &[],
        usage: "move <direction> [amount]",
        description: "Move the list cursor or scroll the view",
    },
    CommandSpec {
        name: "shift",
        aliases: &[],
        usage: "shift <up|down> [amount]",
        description: "Move items in the queue",
    },
    CommandSpec {
        name: "search",
        aliases: &[],
        usage: "search <query>",
        description: "Search Spotify",
    },
    CommandSpec {
        name: "jumpprevious",
        aliases: &[],
        usage: "jumpprevious",
        description: "Jump to the previous search match",
    },
    CommandSpec {
        name: "jumpnext",
        aliases: &[],
        usage: "jumpnext",
        description: "Jump to the next search match",
    },
    CommandSpec {
        name: "jump",
        aliases: &[],
        usage: "jump <query>",
        description: "Jump to a matching row",
    },
    CommandSpec {
        name: "help",
        aliases: &[],
        usage: "help",
        description: "Show keybindings and command help",
    },
    CommandSpec {
        name: "reload",
        aliases: &[],
        usage: "reload",
        description: "Reload the configuration",
    },
    CommandSpec {
        name: "noop",
        aliases: &[],
        usage: "noop",
        description: "Do nothing",
    },
    CommandSpec {
        name: "insert",
        aliases: &[],
        usage: "insert [Spotify URL]",
        description: "Insert a Spotify URL or clipboard contents",
    },
    CommandSpec {
        name: "newplaylist",
        aliases: &[],
        usage: "newplaylist <name>",
        description: "Create a playlist",
    },
    CommandSpec {
        name: "sort",
        aliases: &[],
        usage: "sort <key> [direction]",
        description: "Sort the current playlist",
    },
    CommandSpec {
        name: "logout",
        aliases: &[],
        usage: "logout",
        description: "Remove cached credentials and quit",
    },
    CommandSpec {
        name: "similar",
        aliases: &[],
        usage: "similar <selected|current>",
        description: "Show recommendations",
    },
    CommandSpec {
        name: "redraw",
        aliases: &[],
        usage: "redraw",
        description: "Redraw the screen",
    },
    CommandSpec {
        name: "exec",
        aliases: &[],
        usage: "exec <command>",
        description: "Run a command in the system shell",
    },
    CommandSpec {
        name: "reconnect",
        aliases: &[],
        usage: "reconnect",
        description: "Reconnect to Spotify",
    },
];

static SELECTED_CURRENT: &[&str] = &["selected", "current"];
static REPEAT_VALUES: &[&str] = &[
    "list", "playlist", "queue", "track", "once", "single", "none", "off",
];
static MOVE_VALUES: &[&str] = &[
    "up",
    "down",
    "left",
    "right",
    "playing",
    "top",
    "bottom",
    "leftmost",
    "rightmost",
    "pageup",
    "pagedown",
    "pageleft",
    "pageright",
];
static SORT_KEYS: &[&str] = &["title", "duration", "album", "added", "artist"];
static SORT_DIRECTIONS: &[&str] = &["ascending", "descending", "asc", "desc", "a", "d"];
static FOCUS_TARGETS: &[&str] = &["queue", "search", "library", "cover"];
static SHUFFLE_VALUES: &[&str] = &["on", "off"];
static SHIFT_VALUES: &[&str] = &["up", "down"];
static ADD_VALUES: &[&str] = &["current"];
static SAVE_VALUES: &[&str] = &["current", "queue"];

/// A completion result for the command line.
pub struct Completion {
    /// The command line after completing the common prefix.
    pub completed: String,
    /// Full command line alternatives, including `completed`.
    pub candidates: Vec<String>,
    /// Usage or candidate hint to show beneath the command line.
    pub hint: Option<String>,
}

impl Completion {
    fn unchanged(input: &str) -> Self {
        Self {
            completed: input.to_string(),
            candidates: Vec::new(),
            hint: None,
        }
    }
}

pub fn specs() -> impl Iterator<Item = &'static CommandSpec> {
    COMMAND_SPECS
        .iter()
        .filter(|spec| cfg!(feature = "share_clipboard") || spec.name != "share")
}

pub fn find_spec(command: &str) -> Option<&'static CommandSpec> {
    specs().find(|spec| spec.name == command || spec.aliases.contains(&command))
}

fn command_candidates(prefix: &str) -> Vec<(&'static str, &'static CommandSpec)> {
    let mut candidates = Vec::new();
    for spec in specs() {
        if spec.name.starts_with(prefix) {
            candidates.push((spec.name, spec));
        }
        for alias in spec.aliases {
            if alias.starts_with(prefix) {
                candidates.push((*alias, spec));
            }
        }
    }
    candidates
}

fn argument_values(command: &str, index: usize) -> Option<&'static [&'static str]> {
    match (command, index) {
        ("add", 0) => Some(ADD_VALUES),
        ("save", 0) => Some(SAVE_VALUES),
        ("focus", 0) => Some(FOCUS_TARGETS),
        ("repeat", 0) => Some(REPEAT_VALUES),
        ("shuffle", 0) => Some(SHUFFLE_VALUES),
        ("share", 0) | ("open", 0) | ("similar", 0) => Some(SELECTED_CURRENT),
        ("goto", 0) => Some(&["album", "artist"]),
        ("move", 0) => Some(MOVE_VALUES),
        ("shift", 0) => Some(SHIFT_VALUES),
        ("sort", 0) => Some(SORT_KEYS),
        ("sort", 1) => Some(SORT_DIRECTIONS),
        _ => None,
    }
}

fn common_prefix(values: &[&str]) -> String {
    let Some(first) = values.first() else {
        return String::new();
    };
    let mut prefix: Vec<char> = first.chars().collect();
    for value in &values[1..] {
        let common_len = prefix
            .iter()
            .copied()
            .zip(value.chars())
            .take_while(|(a, b)| a == b)
            .count();
        prefix.truncate(common_len);
        if prefix.is_empty() {
            break;
        }
    }
    prefix.into_iter().collect()
}

/// Complete the command line input using the default `:` prefix.
#[cfg(test)]
pub fn complete(input: &str) -> Completion {
    complete_with_prefix(input, ':')
}

/// Complete a command line that uses `command_prefix` instead of the default `:`.
pub fn complete_with_prefix(input: &str, command_prefix: char) -> Completion {
    let Some(body) = input.strip_prefix(command_prefix) else {
        return Completion::unchanged(input);
    };

    let segment_start = last_command_separator(body).map_or(0, |index| index + 1);
    let before_segment = &body[..segment_start];
    let segment = &body[segment_start..];
    let leading_len = segment.len() - segment.trim_start().len();
    let leading = &segment[..leading_len];
    let trimmed = segment.trim_start();
    let trailing_space = trimmed.chars().last().is_some_and(char::is_whitespace);

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    let base = format!("{command_prefix}{before_segment}{leading}");

    if tokens.is_empty() {
        let candidates: Vec<String> = command_candidates("")
            .into_iter()
            .map(|(name, _)| format!("{base}{name}"))
            .collect();
        return Completion {
            completed: input.to_string(),
            hint: Some(format!("{} commands — Tab completes", candidates.len())),
            candidates,
        };
    }

    let command_token = tokens[0];
    let completing_command = tokens.len() == 1 && !trailing_space;
    if completing_command {
        let matches = command_candidates(command_token);
        if matches.is_empty() {
            let hint = suggest_command(command_token)
                .map(|suggestion| format!("Unknown command. Did you mean: {suggestion}?"));
            return Completion {
                completed: input.to_string(),
                candidates: Vec::new(),
                hint,
            };
        }

        let names: Vec<&str> = matches.iter().map(|(name, _)| *name).collect();
        let prefix = common_prefix(&names);
        let candidates: Vec<String> = names.iter().map(|name| format!("{base}{name}")).collect();
        let hint = if matches.len() == 1 {
            let spec = matches[0].1;
            Some(format!("{} — {}", spec.usage, spec.description))
        } else {
            Some(format!("{} matches: {}", names.len(), names.join(", ")))
        };
        return Completion {
            completed: format!("{base}{prefix}"),
            candidates,
            hint,
        };
    }

    let Some(spec) = find_spec(command_token) else {
        let hint = suggest_command(command_token)
            .map(|suggestion| format!("Unknown command. Did you mean: {suggestion}?"));
        return Completion {
            completed: input.to_string(),
            candidates: Vec::new(),
            hint,
        };
    };

    let argument_index = if trailing_space {
        tokens.len().saturating_sub(1)
    } else {
        tokens.len().saturating_sub(2)
    };
    let current_argument = if trailing_space {
        ""
    } else {
        tokens.last().copied().unwrap_or("")
    };

    let Some(values) = argument_values(spec.name, argument_index) else {
        return Completion {
            completed: input.to_string(),
            candidates: Vec::new(),
            hint: Some(format!("{} — {}", spec.usage, spec.description)),
        };
    };

    let matches: Vec<&str> = values
        .iter()
        .copied()
        .filter(|value| value.starts_with(current_argument))
        .collect();
    if matches.is_empty() {
        return Completion {
            completed: input.to_string(),
            candidates: Vec::new(),
            hint: Some(format!("{} — {}", spec.usage, spec.description)),
        };
    }

    let before_argument = if trailing_space {
        trimmed
    } else {
        &trimmed[..trimmed.len().saturating_sub(current_argument.len())]
    };
    let replacement_base = format!("{base}{before_argument}");
    let prefix = common_prefix(&matches);
    let candidates: Vec<String> = matches
        .iter()
        .map(|value| format!("{replacement_base}{value}"))
        .collect();
    let hint = if matches.len() == 1 {
        Some(format!("{} — {}", spec.usage, spec.description))
    } else {
        Some(format!("{} matches: {}", matches.len(), matches.join(", ")))
    };

    Completion {
        completed: format!("{replacement_base}{prefix}"),
        candidates,
        hint,
    }
}

/// Find the last unescaped `;`, because `;;` represents a literal semicolon.
fn last_command_separator(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut index = 0;
    let mut last = None;
    while index < bytes.len() {
        if bytes[index] == b';' {
            if bytes.get(index + 1) == Some(&b';') {
                index += 2;
                continue;
            }
            last = Some(index);
        }
        index += 1;
    }
    last
}

/// Suggest the closest command or alias for a typo.
pub fn suggest_command(input: &str) -> Option<&'static str> {
    let mut best: Option<(&'static str, usize)> = None;
    for spec in specs() {
        for name in std::iter::once(spec.name).chain(spec.aliases.iter().copied()) {
            if name.starts_with(input) {
                return Some(name);
            }
            let distance = edit_distance(input, name);
            if best.is_none_or(|(_, best_distance)| distance < best_distance) {
                best = Some((name, distance));
            }
        }
    }
    best.filter(|(name, distance)| *distance <= 2 && name.len().abs_diff(input.len()) <= 2)
        .map(|(name, _)| name)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b_chars: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b_chars.len()).collect();
    let mut current = vec![0; b_chars.len() + 1];
    for (i, a_char) in a.chars().enumerate() {
        current[0] = i + 1;
        for (j, b_char) in b_chars.iter().enumerate() {
            let substitution = previous[j] + usize::from(a_char != *b_char);
            current[j + 1] = (current[j] + 1).min(previous[j + 1] + 1).min(substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b_chars.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_a_unique_command() {
        let result = complete(":foc");
        assert_eq!(result.completed, ":focus");
        assert_eq!(result.candidates, vec![":focus"]);
    }

    #[test]
    fn completes_with_a_custom_command_key() {
        let result = complete_with_prefix(">foc", '>');
        assert_eq!(result.completed, ">focus");
    }

    #[test]
    fn completes_a_unique_alias() {
        let result = complete(":tog");
        assert_eq!(result.completed, ":toggleplay");
    }

    #[test]
    fn completes_a_common_prefix() {
        let result = complete(":vol");
        assert_eq!(result.completed, ":vol");
        assert!(result.candidates.contains(&":volup".to_string()));
        assert!(result.candidates.contains(&":voldown".to_string()));
    }

    #[test]
    fn completes_command_arguments() {
        let result = complete(":focus li");
        assert_eq!(result.completed, ":focus library");
        let result = complete(":repeat tr");
        assert_eq!(result.completed, ":repeat track");
        let result = complete(":sort title desce");
        assert_eq!(result.completed, ":sort title descending");
    }

    #[test]
    fn completes_arguments_after_a_separator() {
        let result = complete(":shuffle on; focus se");
        assert_eq!(result.completed, ":shuffle on; focus search");
    }

    #[test]
    fn completed_commands_show_usage() {
        let result = complete(":focus");
        assert!(result.hint.as_deref().unwrap_or("").contains("focus"));
        let result = complete(":foucs");
        assert!(result.hint.unwrap_or_default().contains("Did you mean"));
    }

    #[test]
    fn suggests_typos() {
        assert_eq!(suggest_command("foucs"), Some("focus"));
    }

    #[test]
    fn every_parser_command_has_completion_metadata() {
        for command in crate::command::ALL_COMMANDS {
            let name = command.split_whitespace().next().unwrap_or(command);
            assert!(
                find_spec(name).is_some(),
                "missing completion metadata for {command}"
            );
        }
    }

    #[test]
    fn escape_sequences_are_not_treated_as_separators() {
        assert_eq!(last_command_separator("a;;b"), None);
        assert_eq!(last_command_separator("a;b"), Some(1));
    }
}
