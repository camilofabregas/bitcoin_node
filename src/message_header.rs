use crate::errors::RustifyError;
use bitcoin_hashes::{sha256d, Hash};

pub const MESSAGE_HEADER_SIZE: usize = 24;
const TESTNET_START_STRING: [u8; 4] = [0x0B, 0x11, 0x09, 0x07];

#[derive(Debug, Clone)]
pub struct MessageHeader {
    pub start_string: [u8; 4],
    pub command_name: [u8; 12],
    pub payload_size: u32,
    pub checksum: [u8; 4],
}

impl MessageHeader {
    pub fn new(command: String, payload: &[u8]) -> MessageHeader {
        MessageHeader {
            start_string: TESTNET_START_STRING,
            command_name: MessageHeader::procesar_comando(command),
            payload_size: payload.len() as u32,
            checksum: MessageHeader::procesar_payload(payload),
        }
    }

    fn procesar_payload(payload: &[u8]) -> [u8; 4] {
        let hash = sha256d::Hash::hash(payload).to_byte_array();
        let hash_slice = &hash[..4];
        let mut checksum: [u8; 4] = [0; 4];
        checksum.copy_from_slice(hash_slice);
        checksum
    }

    fn procesar_comando(string: String) -> [u8; 12] {
        let mut bytes: [u8; 12] = [b'\0'; 12];
        bytes[..string.len()].copy_from_slice(string.as_bytes());
        bytes
    }

    pub fn as_bytes(&self) -> [u8; 24] {
        let mut bytes = [0u8; 24];
        bytes[..4].copy_from_slice(&self.start_string);
        bytes[4..16].copy_from_slice(&self.command_name);
        bytes[16..20].copy_from_slice(&self.payload_size.to_le_bytes());
        bytes[20..].copy_from_slice(&self.checksum);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<MessageHeader, RustifyError> {
        let mut start_string = [0; 4];
        start_string.copy_from_slice(&bytes[0..4]);

        let mut command_name = [0; 12];
        command_name.copy_from_slice(&bytes[4..16]);

        let payload_size = u32::from_le_bytes(bytes[16..20].try_into()?);

        let mut checksum = [0; 4];
        checksum.copy_from_slice(&bytes[20..24]);

        Ok(MessageHeader {
            start_string,
            command_name,
            payload_size,
            checksum,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_header() -> Result<(), RustifyError> {
        // Create a new message header
        let command = "version".to_owned();
        let payload = vec![1, 2, 3, 4];
        let header = MessageHeader::new(command, &payload);

        // Convert the header to bytes
        let bytes = header.as_bytes();

        // Create a new header from the bytes
        let new_header = MessageHeader::from_bytes(&bytes)?;

        // Ensure that the new header is equal to the original header
        assert_eq!(header.start_string, new_header.start_string);
        assert_eq!(header.command_name, new_header.command_name);
        assert_eq!(header.payload_size, new_header.payload_size);
        assert_eq!(header.checksum, new_header.checksum);

        Ok(())
    }

    #[test]
    fn test_procesar_payload() {
        let payload = b"hello world";
        let expected_result: [u8; 4] = [0xbc, 0x62, 0xd4, 0xb8];
        let result = MessageHeader::procesar_payload(payload);

        let payload_vacio = &[];
        let expected_result_vacio: [u8; 4] = [0x5d, 0xf6, 0xe0, 0xe2];
        let result_vacio = MessageHeader::procesar_payload(payload_vacio);

        assert_eq!(result, expected_result);
        assert_eq!(result_vacio, expected_result_vacio);
    }

    #[test]
    fn test_procesar_comando() {
        let string = String::from("version");
        let expected_result: [u8; 12] = [118, 101, 114, 115, 105, 111, 110, 0, 0, 0, 0, 0];
        let result = MessageHeader::procesar_comando(string);
        assert_eq!(result, expected_result);
    }
}
