//! Turns raw terminal bytes into game events.

use crate::game::Direction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Move(Direction),
    Quit,
    New,
    Continue,
    Press(i32, i32),
    Release(i32, i32),
}

const ESC: u8 = 0x1b;

fn is_csi_param(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b';' | b'<' | b'?')
}

fn is_final(byte: u8) -> bool {
    (b'@'..=b'~').contains(&byte)
}

fn arrow(byte: u8) -> Option<Event> {
    match byte {
        b'A' => Some(Event::Move(Direction::Up)),
        b'B' => Some(Event::Move(Direction::Down)),
        b'C' => Some(Event::Move(Direction::Right)),
        b'D' => Some(Event::Move(Direction::Left)),
        _ => None,
    }
}

fn key(byte: u8) -> Option<Event> {
    match byte {
        b'q' | b'Q' | 0x03 => Some(Event::Quit),
        b'n' | b'N' => Some(Event::New),
        b'c' | b'C' => Some(Event::Continue),
        _ => None,
    }
}

/// Parses SGR mouse parameters, the part of `ESC [ < b ; x ; y M` between `<` and the final byte.
fn mouse_params(params: &[u8]) -> Option<(i32, i32, i32)> {
    let mut fields = params
        .strip_prefix(b"<")?
        .split(|&byte| byte == b';')
        .map(|field| {
            if field.is_empty() || !field.iter().all(u8::is_ascii_digit) {
                return None;
            }
            std::str::from_utf8(field).ok()?.parse::<i32>().ok()
        });
    let parsed = (fields.next()??, fields.next()??, fields.next()??);
    fields.next().is_none().then_some(parsed)
}

fn mouse_event(button: i32, x: i32, y: i32, final_byte: u8) -> Option<Event> {
    match (button, final_byte) {
        // Primary button only: this ignores the wheel and motion.
        (0, b'M') => Some(Event::Press(x, y)),
        (0 | 3, b'm') => Some(Event::Release(x, y)),
        _ => None,
    }
}

/// Returns the events found and any trailing bytes of an incomplete sequence.
pub fn parse_input(data: &[u8]) -> (Vec<Event>, Vec<u8>) {
    let mut events = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if data[i] != ESC {
            events.extend(key(data[i]));
            i += 1;
            continue;
        }
        match data.get(i + 1) {
            None => return (events, data[i..].to_vec()),
            Some(b'[') => {
                let start = i + 2;
                let params = data[start..]
                    .iter()
                    .take_while(|&&byte| is_csi_param(byte))
                    .count();
                let end = start + params;
                match data.get(end) {
                    None => return (events, data[i..].to_vec()),
                    Some(&last) if is_final(last) => {
                        let mouse = matches!(last, b'M' | b'm')
                            .then(|| mouse_params(&data[start..end]))
                            .flatten();
                        match mouse {
                            Some((button, x, y)) => events.extend(mouse_event(button, x, y, last)),
                            None => events.extend(arrow(last)),
                        }
                        i = end + 1;
                    }
                    Some(_) => i += 1, // Stray ESC.
                }
            }
            Some(b'O') => match data.get(i + 2) {
                None => return (events, data[i..].to_vec()),
                Some(&last) if is_final(last) => {
                    events.extend(arrow(last));
                    i += 3;
                }
                Some(_) => i += 1, // Stray ESC.
            },
            Some(_) => i += 1, // Stray ESC.
        }
    }
    (events, Vec::new())
}

/// Maps a drag to a direction. Terminal cells are about twice as tall as wide.
pub fn swipe_direction(x0: i32, y0: i32, x1: i32, y1: i32) -> Option<Direction> {
    let (dx, dy) = (x1 - x0, (y1 - y0) * 2);
    if dx.abs().max(dy.abs()) < 3 {
        None
    } else if dx.abs() >= dy.abs() {
        Some(if dx > 0 {
            Direction::Right
        } else {
            Direction::Left
        })
    } else {
        Some(if dy > 0 {
            Direction::Down
        } else {
            Direction::Up
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Direction::*;
    use super::Event::*;
    use super::*;

    fn events(data: &[u8]) -> Vec<Event> {
        parse_input(data).0
    }

    #[test]
    fn arrows_in_both_cursor_modes() {
        assert_eq!(
            events(b"\x1b[A\x1b[B\x1b[C\x1b[D"),
            [Move(Up), Move(Down), Move(Right), Move(Left)]
        );
        assert_eq!(events(b"\x1bOA\x1bOD"), [Move(Up), Move(Left)]);
    }

    #[test]
    fn keys() {
        assert_eq!(events(b"qnc\x03x"), [Quit, New, Continue, Quit]);
    }

    #[test]
    fn split_sequence_is_kept() {
        let (found, rest) = parse_input(b"\x1b[");
        assert!(found.is_empty());
        assert_eq!(rest, b"\x1b[");
        let joined = [rest.as_slice(), b"A"].concat();
        assert_eq!(parse_input(&joined), (vec![Move(Up)], vec![]));
    }

    #[test]
    fn split_mouse_sequence_is_kept() {
        let (found, rest) = parse_input(b"\x1b[<0;12");
        assert!(found.is_empty());
        assert_eq!(rest, b"\x1b[<0;12");
        let joined = [rest.as_slice(), b";5M"].concat();
        assert_eq!(events(&joined), [Press(12, 5)]);
    }

    #[test]
    fn mouse_press_and_release() {
        assert_eq!(
            events(b"\x1b[<0;10;5M\x1b[<0;20;5m"),
            [Press(10, 5), Release(20, 5)]
        );
    }

    #[test]
    fn wheel_is_ignored() {
        assert!(events(b"\x1b[<64;10;5M\x1b[<65;10;5M").is_empty());
    }

    #[test]
    fn unknown_sequences_do_not_trigger_keys() {
        assert!(events(b"\x1b[200~").is_empty());
    }

    #[test]
    fn swipe() {
        assert_eq!(swipe_direction(10, 10, 20, 10), Some(Right));
        assert_eq!(swipe_direction(20, 10, 10, 10), Some(Left));
        assert_eq!(swipe_direction(10, 10, 10, 15), Some(Down));
        assert_eq!(swipe_direction(10, 15, 10, 10), Some(Up));
    }

    #[test]
    fn tap_is_not_a_swipe() {
        assert_eq!(swipe_direction(10, 10, 10, 10), None);
        assert_eq!(swipe_direction(10, 10, 11, 10), None);
    }
}
