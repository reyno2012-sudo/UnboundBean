pub const SERVICE_UUID: &str = "020cf5da-0000-1000-8000-00805f9b34fb";
pub const WRITE_UUID: &str = "00007777-0000-1000-8000-00805f9b34fb";
pub const NOTIFY_UUID: &str = "00008888-0000-1000-8000-00805f9b34fb";

pub fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte))
}

pub fn build_command(command_type: u8, command_id: u8, payload: &[u8]) -> Vec<u8> {
    let total_length = 10 + payload.len();
    let mut frame = Vec::with_capacity(total_length);
    frame.extend_from_slice(&[0x08, 0xEE, 0x00, 0x00, 0x00, command_type, command_id]);
    frame.extend_from_slice(&(total_length as u16).to_le_bytes());
    frame.extend_from_slice(payload);
    frame.push(checksum(&frame));
    frame
}

pub fn is_valid_response(frame: &[u8]) -> bool {
    if frame.len() < 10 || frame[0..4] != [0x09, 0xFF, 0x00, 0x00] {
        return false;
    }
    let expected_length = u16::from_le_bytes([frame[7], frame[8]]) as usize;
    expected_length == frame.len() && checksum(&frame[..frame.len() - 1]) == frame[frame.len() - 1]
}

#[derive(Default)]
pub struct FrameDecoder {
    buffer: Vec<u8>,
}

impl FrameDecoder {
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.buffer.extend_from_slice(bytes);
        let mut decoded = Vec::new();

        loop {
            let Some(start) = self
                .buffer
                .windows(4)
                .position(|window| window == [0x09, 0xFF, 0x00, 0x00])
            else {
                const MAGIC: [u8; 4] = [0x09, 0xFF, 0x00, 0x00];
                let keep = (1..=self.buffer.len().min(MAGIC.len() - 1))
                    .rev()
                    .find(|length| self.buffer[self.buffer.len() - length..] == MAGIC[..*length])
                    .unwrap_or(0);
                let discard = self.buffer.len() - keep;
                self.buffer.drain(..discard);
                break;
            };

            if start > 0 {
                self.buffer.drain(..start);
            }
            if self.buffer.len() < 9 {
                break;
            }

            let length = u16::from_le_bytes([self.buffer[7], self.buffer[8]]) as usize;
            if length < 10 {
                self.buffer.remove(0);
                continue;
            }
            if self.buffer.len() < length {
                break;
            }

            let candidate: Vec<u8> = self.buffer.drain(..length).collect();
            if is_valid_response(&candidate) {
                decoded.push(candidate);
            }
        }

        decoded
    }
}

pub fn response_matches(frame: &[u8], command_type: u8, command_id: u8) -> bool {
    frame.len() >= 10 && frame[5] == command_type && frame[6] == command_id
}

pub fn response_succeeded(frame: &[u8]) -> bool {
    frame.get(4).is_some_and(|status| status & 0x0F == 1)
}
