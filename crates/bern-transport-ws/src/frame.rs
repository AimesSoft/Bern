//! WebSocket framing (RFC 6455 §5) for the server side: client frames
//! must be masked, server frames are not. Messages are text; binary and
//! oversized messages close the connection.

use std::io::{self, Read, Write};

/// Largest message the server accepts from a page.
pub const MAX_MESSAGE: usize = 16 << 20;

pub enum Incoming {
    Text(String),
    Close,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

struct Frame {
    fin: bool,
    opcode: u8,
    payload: Vec<u8>,
}

fn read_frame(stream: &mut impl Read) -> io::Result<Frame> {
    let mut head = [0u8; 2];
    stream.read_exact(&mut head)?;
    let fin = head[0] & 0x80 != 0;
    if head[0] & 0x70 != 0 {
        return Err(invalid("reserved bits are set"));
    }
    let opcode = head[0] & 0x0f;
    if head[1] & 0x80 == 0 {
        return Err(invalid("client frames must be masked"));
    }
    let length = match head[1] & 0x7f {
        126 => {
            let mut bytes = [0u8; 2];
            stream.read_exact(&mut bytes)?;
            u64::from(u16::from_be_bytes(bytes))
        }
        127 => {
            let mut bytes = [0u8; 8];
            stream.read_exact(&mut bytes)?;
            u64::from_be_bytes(bytes)
        }
        short => u64::from(short),
    };
    if length > MAX_MESSAGE as u64 {
        return Err(invalid("message too large"));
    }
    if opcode >= 0x8 && (length > 125 || !fin) {
        return Err(invalid("invalid control frame"));
    }
    let mut mask = [0u8; 4];
    stream.read_exact(&mut mask)?;
    let mut payload = vec![0u8; length as usize];
    stream.read_exact(&mut payload)?;
    for (index, byte) in payload.iter_mut().enumerate() {
        *byte ^= mask[index % 4];
    }
    Ok(Frame {
        fin,
        opcode,
        payload,
    })
}

/// Write one unmasked server frame.
pub fn write_frame(stream: &mut impl Write, opcode: u8, payload: &[u8]) -> io::Result<()> {
    let mut head = vec![0x80 | opcode];
    match payload.len() {
        length @ 0..=125 => head.push(length as u8),
        length @ 126..=0xffff => {
            head.push(126);
            head.extend_from_slice(&(length as u16).to_be_bytes());
        }
        length => {
            head.push(127);
            head.extend_from_slice(&(length as u64).to_be_bytes());
        }
    }
    // One write per frame, so frames from different threads never mix.
    head.extend_from_slice(payload);
    stream.write_all(&head)?;
    stream.flush()
}

/// Read the next message, answering pings on the way.
pub fn read_message(stream: &mut (impl Read + Write)) -> io::Result<Incoming> {
    let mut message: Vec<u8> = Vec::new();
    let mut started = false;
    loop {
        let frame = read_frame(stream)?;
        match frame.opcode {
            0x9 => write_frame(stream, 0xA, &frame.payload)?,
            0xA => {}
            0x8 => {
                let _ = write_frame(stream, 0x8, &frame.payload);
                return Ok(Incoming::Close);
            }
            0x1 if !started => {
                started = true;
                message = frame.payload;
            }
            0x0 if started => {
                if message.len() + frame.payload.len() > MAX_MESSAGE {
                    return Err(invalid("message too large"));
                }
                message.extend_from_slice(&frame.payload);
            }
            0x2 => return Err(invalid("binary messages are not accepted")),
            _ => return Err(invalid("unexpected frame")),
        }
        // Control frames may arrive between fragments; only a final data
        // frame ends the message.
        if started && frame.fin && frame.opcode <= 0x1 {
            return String::from_utf8(message)
                .map(Incoming::Text)
                .map_err(|_| invalid("text is not UTF-8"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A masked client frame.
    fn client(opcode: u8, fin: bool, payload: &[u8]) -> Vec<u8> {
        let mask = [0x12, 0x34, 0x56, 0x78];
        let mut bytes = vec![if fin { 0x80 } else { 0 } | opcode];
        match payload.len() {
            length @ 0..=125 => bytes.push(0x80 | length as u8),
            length => {
                bytes.push(0x80 | 126);
                bytes.extend_from_slice(&(length as u16).to_be_bytes());
            }
        }
        bytes.extend_from_slice(&mask);
        bytes.extend(
            payload
                .iter()
                .enumerate()
                .map(|(i, byte)| byte ^ mask[i % 4]),
        );
        bytes
    }

    struct Duplex {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }

    impl Read for Duplex {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.input.read(buffer)
        }
    }

    impl Write for Duplex {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.output.write(buffer)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn duplex(bytes: Vec<u8>) -> Duplex {
        Duplex {
            input: Cursor::new(bytes),
            output: Vec::new(),
        }
    }

    #[test]
    fn fragmented_text_reassembles_and_pings_are_answered() {
        let mut bytes = client(0x1, false, b"hel");
        bytes.extend(client(0x9, true, b"p"));
        bytes.extend(client(0x0, true, b"lo"));
        let long = "x".repeat(300);
        bytes.extend(client(0x1, true, long.as_bytes()));
        let mut stream = duplex(bytes);
        assert!(
            matches!(read_message(&mut stream).unwrap(), Incoming::Text(text) if text == "hello")
        );
        assert_eq!(stream.output, vec![0x8A, 1, b'p']);
        assert!(matches!(read_message(&mut stream).unwrap(), Incoming::Text(text) if text == long));
    }

    #[test]
    fn unmasked_binary_and_close_frames_are_handled() {
        let mut unmasked = vec![0x81, 2];
        unmasked.extend_from_slice(b"hi");
        assert!(read_message(&mut duplex(unmasked)).is_err());
        assert!(read_message(&mut duplex(client(0x2, true, b"x"))).is_err());
        let mut stream = duplex(client(0x8, true, &[0x03, 0xe8]));
        assert!(matches!(
            read_message(&mut stream).unwrap(),
            Incoming::Close
        ));
        assert_eq!(stream.output, vec![0x88, 2, 0x03, 0xe8]);
    }

    #[test]
    fn server_frames_use_the_shortest_length() {
        let mut out = Vec::new();
        write_frame(&mut out, 0x1, b"ok").unwrap();
        assert_eq!(out, vec![0x81, 2, b'o', b'k']);
        let mut out = Vec::new();
        write_frame(&mut out, 0x1, &[0u8; 200]).unwrap();
        assert_eq!(&out[..4], &[0x81, 126, 0, 200]);
    }
}
