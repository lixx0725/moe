//! 将控制键转换为终端序列，普通文字通过输入法接口提交。

use gpui::Keystroke;

const CSI: &[u8] = b"\x1b[";
const SS3: &[u8] = b"\x1bO";
const ESC: u8 = 0x1b;

pub(crate) fn encode(stroke: &Keystroke, application_cursor: bool) -> Option<Vec<u8>> {
    let modifiers = stroke.modifiers;
    if modifiers.platform {
        return None;
    }
    let name = stroke.key.as_str();
    let parameter = 1
        + u8::from(modifiers.shift)
        + u8::from(modifiers.alt) * 2
        + u8::from(modifiers.control) * 4;

    let cursor = match name {
        "up" => Some(b'A'),
        "down" => Some(b'B'),
        "right" => Some(b'C'),
        "left" => Some(b'D'),
        "home" => Some(b'H'),
        "end" => Some(b'F'),
        _ => None,
    };
    if let Some(final_byte) = cursor {
        let plain_prefix = if application_cursor { SS3 } else { CSI };
        return Some(letter_sequence(final_byte, parameter, plain_prefix));
    }
    if let Some(key) = function_key(name) {
        return Some(if key <= 4 {
            letter_sequence(b'P' + key - 1, parameter, SS3)
        } else {
            numbered_sequence(
                [15, 17, 18, 19, 20, 21, 23, 24][usize::from(key - 5)],
                parameter,
            )
        });
    }
    let number = match name {
        "insert" => Some(2),
        "delete" => Some(3),
        "pageup" => Some(5),
        "pagedown" => Some(6),
        _ => None,
    };
    if let Some(number) = number {
        return Some(numbered_sequence(number, parameter));
    }
    if name == "tab" && modifiers.shift {
        return Some(b"\x1b[Z".to_vec());
    }
    let byte = match name {
        "enter" => Some(b'\r'),
        "backspace" => Some(if modifiers.control { 8 } else { 127 }),
        "tab" => Some(b'\t'),
        "escape" => return Some(vec![ESC]),
        _ if modifiers.control => control_byte(name),
        _ => None,
    };
    if let Some(byte) = byte {
        return Some(if modifiers.alt {
            vec![ESC, byte]
        } else {
            vec![byte]
        });
    }
    if modifiers.alt && !modifiers.control {
        let text = stroke.key_char.as_deref()?;
        return Some([&[ESC], text.as_bytes()].concat());
    }
    None
}

fn function_key(name: &str) -> Option<u8> {
    let number = name.strip_prefix('f')?.parse::<u8>().ok()?;
    (1..=12).contains(&number).then_some(number)
}

fn letter_sequence(letter: u8, parameter: u8, prefix: &[u8]) -> Vec<u8> {
    // 光标应用模式只改变无修饰键的前缀，带修饰键统一使用 CSI 1;<参数>。
    let mut bytes = if parameter == 1 {
        prefix.to_vec()
    } else {
        format!("\x1b[1;{parameter}").into_bytes()
    };
    bytes.push(letter);
    bytes
}

fn numbered_sequence(number: u8, parameter: u8) -> Vec<u8> {
    if parameter > 1 {
        format!("\x1b[{number};{parameter}~").into_bytes()
    } else {
        format!("\x1b[{number}~").into_bytes()
    }
}

fn control_byte(name: &str) -> Option<u8> {
    match name {
        "space" | "@" | "2" => Some(0),
        "3" => Some(27),
        "4" => Some(28),
        "5" => Some(29),
        "6" => Some(30),
        "-" | "7" => Some(31),
        "8" | "?" => Some(127),
        _ => match name.as_bytes() {
            [character @ (b'a'..=b'z' | b'['..=b'_')] => Some(*character & 0x1f),
            _ => None,
        },
    }
}

pub(crate) fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(text.len() + if bracketed { 12 } else { 0 });
    if bracketed {
        bytes.extend_from_slice(b"\x1b[200~");
    }
    let mut source = text.bytes().peekable();
    while let Some(byte) = source.next() {
        match byte {
            b'\r' => {
                source.next_if_eq(&b'\n');
                bytes.push(b'\r');
            }
            b'\n' => bytes.push(b'\r'),
            // 粘贴内容中的 ESC 不能提前结束括号粘贴。
            ESC if bracketed => {}
            byte => bytes.push(byte),
        }
    }
    if bracketed {
        bytes.extend_from_slice(b"\x1b[201~");
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_and_terminal_modes_encode_expected_bytes() {
        for (key, application_cursor, expected) in [
            ("ctrl-c", false, b"\x03".as_slice()),
            ("ctrl-[", false, b"\x1b"),
            ("up", false, b"\x1b[A"),
            ("up", true, b"\x1bOA"),
            ("ctrl-shift-left", true, b"\x1b[1;6D"),
            ("alt-f5", false, b"\x1b[15;3~"),
        ] {
            let stroke = Keystroke::parse(key).unwrap();
            assert_eq!(
                encode(&stroke, application_cursor).as_deref(),
                Some(expected)
            );
        }
    }

    #[test]
    fn plain_text_stays_in_the_native_input_path() {
        assert_eq!(encode(&Keystroke::parse("a").unwrap(), false), None);
        assert_eq!(encode(&Keystroke::parse("cmd-v").unwrap(), false), None);
    }

    #[test]
    fn bracketed_paste_normalizes_newlines_and_protects_its_delimiters() {
        assert_eq!(paste("目录\r\n文件\n", false), "目录\r文件\r".as_bytes());
        assert_eq!(paste("a\x1bb", true), b"\x1b[200~ab\x1b[201~");
    }
}
