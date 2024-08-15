use crate::errors::RustifyError;
use crate::logger::{log, Action, Lvl};
use crate::message_handler::handle_specific_message;
use crate::node::write_to_node;
use crate::{config::Config, message_header::MessageHeader};
use chrono::Utc;
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::mpsc::Sender;

const VERSION_SIZE: usize = 110;

#[derive(Debug)]
pub struct VersionMessage {
    pub version: i32,
    pub services: u64,
    pub timestamp: i64,
    pub receiver_services: u64,
    pub receiver_address: [u8; 16],
    pub receiver_port: u16,
    pub sender_services: u64,
    pub sender_address: [u8; 16],
    pub sender_port: u16,
    pub nonce: u64,
    pub user_agent_bytes: u8,
    pub user_agent: Vec<u8>,
    pub start_height: i32,
    pub relay: u8,
}

impl VersionMessage {
    pub fn new(receiver: SocketAddr, sender: SocketAddr, config: &Config) -> VersionMessage {
        VersionMessage {
            version: config.version,
            services: config.node_network_limited,
            timestamp: Utc::now().timestamp(),
            receiver_services: config.node_network,
            receiver_address: VersionMessage::procesar_ip(receiver.ip()),
            receiver_port: receiver.port(),
            sender_services: config.node_network_limited,
            sender_address: VersionMessage::procesar_ip(sender.ip()),
            sender_port: sender.port(),
            nonce: 0x00,
            user_agent_bytes: config.user_agent_rustify.len() as u8,
            user_agent: config.user_agent_rustify.as_bytes().to_vec(),
            start_height: 0x00,
            relay: 0x01,
        }
    }

    pub fn procesar_ip(ip: IpAddr) -> [u8; 16] {
        match ip {
            IpAddr::V4(ip) => {
                let octetos = ip.octets();
                [
                    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xff, 0xff, octetos[0],
                    octetos[1], octetos[2], octetos[3],
                ]
            }
            IpAddr::V6(ip) => ip.octets(),
        }
    }

    pub fn as_bytes(&self, config_user_agent_rustify: &str) -> [u8; VERSION_SIZE] {
        let mut result: [u8; VERSION_SIZE] = [0; VERSION_SIZE];
        let mut index = 0;
        result[index..index + 4].copy_from_slice(&self.version.to_le_bytes());
        index += 4;
        result[index..index + 8].copy_from_slice(&self.services.to_le_bytes());
        index += 8;
        result[index..index + 8].copy_from_slice(&self.timestamp.to_le_bytes());
        index += 8;
        result[index..index + 8].copy_from_slice(&self.receiver_services.to_le_bytes());
        index += 8;
        result[index..index + 16].copy_from_slice(&self.receiver_address);
        index += 16;
        result[index..index + 2].copy_from_slice(&self.receiver_port.to_be_bytes());
        index += 2;
        result[index..index + 8].copy_from_slice(&self.sender_services.to_le_bytes());
        index += 8;
        result[index..index + 16].copy_from_slice(&self.sender_address);
        index += 16;
        result[index..index + 2].copy_from_slice(&self.sender_port.to_be_bytes());
        index += 2;
        result[index..index + 8].copy_from_slice(&self.nonce.to_le_bytes());
        index += 8;
        result[index] = self.user_agent_bytes;
        index += 1;
        result[index..index + config_user_agent_rustify.len()].copy_from_slice(&self.user_agent);
        index += config_user_agent_rustify.len();
        result[index..index + 4].copy_from_slice(&self.start_height.to_le_bytes());
        index += 4;
        result[index..index + 1].copy_from_slice(&self.relay.to_le_bytes());
        result
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<VersionMessage, RustifyError> {
        let mut index = 0;
        let version = i32::from_le_bytes(bytes[index..index + 4].try_into()?);
        index += 4;
        let services = u64::from_le_bytes(bytes[index..index + 8].try_into()?);
        index += 8;
        let timestamp = i64::from_le_bytes(bytes[index..index + 8].try_into()?);
        index += 8;
        let receiver_services = u64::from_le_bytes(bytes[index..index + 8].try_into()?);
        index += 8;
        let mut receiver_address = [0u8; 16];
        receiver_address.copy_from_slice(&bytes[index..index + 16]);
        index += 16;
        let receiver_port = u16::from_be_bytes(bytes[index..index + 2].try_into()?);
        index += 2;
        let sender_services = u64::from_le_bytes(bytes[index..index + 8].try_into()?);
        index += 8;
        let mut sender_address = [0u8; 16];
        sender_address.copy_from_slice(&bytes[index..index + 16]);
        index += 16;
        let sender_port = u16::from_be_bytes(bytes[index..index + 2].try_into()?);
        index += 2;
        let nonce = u64::from_le_bytes(bytes[index..index + 8].try_into()?);
        index += 8;
        let user_agent_bytes = bytes[index];
        index += 1;
        let user_agent = bytes[index..index + (user_agent_bytes as usize)].to_vec();
        index += user_agent_bytes as usize;
        let start_height = i32::from_le_bytes(bytes[index..index + 4].try_into()?);
        index += 4;
        let relay = bytes[index];
        Ok(VersionMessage {
            version,
            services,
            timestamp,
            receiver_services,
            receiver_address,
            receiver_port,
            sender_services,
            sender_address,
            sender_port,
            nonce,
            user_agent_bytes,
            user_agent,
            start_height,
            relay,
        })
    }
}

/// Envío y recepción de mensajes version para el handshake del nodo.
pub fn version(
    socket: &mut TcpStream,
    config: &Config,
    logger_sender: &Sender<String>,
) -> Result<(), RustifyError> {
    let version_message = VersionMessage::new(socket.peer_addr()?, socket.local_addr()?, config);
    let version_message_bytes = version_message.as_bytes(&config.user_agent_rustify);

    let version_message_header = MessageHeader::new("version".to_string(), &version_message_bytes);
    let version_message_header_bytes = version_message_header.as_bytes();

    write_to_node(
        socket,
        &version_message_header_bytes,
        &version_message_bytes,
    )?;
    log(
        Lvl::Info(Action::CONNECT),
        "Enviado mensaje version.",
        logger_sender,
    );

    handle_specific_message(socket, "version\0\0\0\0\0".to_string(), logger_sender)?;

    Ok(())
}

/// Envío y recepción de mensajes verack para el handshake del nodo.
pub fn verack(socket: &mut TcpStream, logger_sender: &Sender<String>) -> Result<(), RustifyError> {
    let verack_message_header = MessageHeader::new("verack".to_string(), &[]);
    let verack_message_header_bytes = verack_message_header.as_bytes();

    write_to_node(socket, &verack_message_header_bytes, &[])?;
    log(
        Lvl::Info(Action::CONNECT),
        "Enviado mensaje verack.",
        logger_sender,
    );

    handle_specific_message(socket, "verack\0\0\0\0\0\0".to_string(), logger_sender)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_bytes() {
        let message = VersionMessage {
            version: 536870912,
            services: 1,
            timestamp: 123456789,
            receiver_services: 1,
            receiver_address: [0; 16],
            receiver_port: 8333,
            sender_services: 1,
            sender_address: [0; 16],
            sender_port: 8333,
            nonce: 123456789,
            user_agent_bytes: 24,
            user_agent: "/asdasdasdasdasdasdasda/".as_bytes().to_vec(),
            start_height: 123456,
            relay: 0,
        };

        let result = message.as_bytes(&"/asdasdasdasdasdasdasda/");
        let expected: [u8; VERSION_SIZE] = [
            0x00, 0x00, 0x00, 0x20, // version
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // services
            0x15, 0xCD, 0x5B, 0x07, 0x00, 0x00, 0x00, 0x00, // timestamp
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // receiver_services
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, // receiver_address
            0x20, 0x8D, // receiver_port
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // sender_services
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, // sender_address
            0x20, 0x8D, // sender_port
            0x15, 0xCD, 0x5B, 0x07, 0x00, 0x00, 0x00, 0x00, // nonce
            0x18, // user_agent_bytes
            0x2f, 0x61, 0x73, 0x64, 0x61, 0x73, 0x64, 0x61, 0x73, 0x64, 0x61, 0x73, 0x64, 0x61,
            0x73, 0x64, 0x61, 0x73, 0x64, 0x61, 0x73, 0x64, 0x61, 0x2f, // user_agent
            0x40, 0xE2, 0x01, 0x00, // start_height
            0x00, // relay
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_from_bytes() -> Result<(), RustifyError> {
        let message = VersionMessage {
            version: 70015,
            services: 1,
            timestamp: 1555347006,
            receiver_services: 1,
            receiver_address: [127, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            receiver_port: 8333,
            sender_services: 1,
            sender_address: [127, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            sender_port: 8333,
            nonce: 0,
            user_agent_bytes: 0,
            user_agent: Vec::new(),
            start_height: 0,
            relay: 0,
        };

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&message.version.to_le_bytes());
        bytes.extend_from_slice(&message.services.to_le_bytes());
        bytes.extend_from_slice(&message.timestamp.to_le_bytes());
        bytes.extend_from_slice(&message.receiver_services.to_le_bytes());
        bytes.extend_from_slice(&message.receiver_address);
        bytes.extend_from_slice(&message.receiver_port.to_be_bytes());
        bytes.extend_from_slice(&message.sender_services.to_le_bytes());
        bytes.extend_from_slice(&message.sender_address);
        bytes.extend_from_slice(&message.sender_port.to_be_bytes());
        bytes.extend_from_slice(&message.nonce.to_le_bytes());
        bytes.extend_from_slice(&[message.user_agent_bytes]);
        bytes.extend_from_slice(&message.start_height.to_le_bytes());
        bytes.push(message.relay);

        let decoded_message = VersionMessage::from_bytes(&bytes)?;

        assert_eq!(decoded_message.version, message.version);
        assert_eq!(decoded_message.services, message.services);
        assert_eq!(decoded_message.timestamp, message.timestamp);
        assert_eq!(decoded_message.receiver_services, message.receiver_services);
        assert_eq!(decoded_message.receiver_address, message.receiver_address);
        assert_eq!(decoded_message.receiver_port, message.receiver_port);
        assert_eq!(decoded_message.sender_services, message.sender_services);
        assert_eq!(decoded_message.sender_address, message.sender_address);
        assert_eq!(decoded_message.sender_port, message.sender_port);
        assert_eq!(decoded_message.nonce, message.nonce);
        assert_eq!(decoded_message.user_agent_bytes, message.user_agent_bytes);
        assert_eq!(decoded_message.user_agent, message.user_agent);
        assert_eq!(decoded_message.start_height, message.start_height);
        assert_eq!(decoded_message.relay, message.relay);

        Ok(())
    }

    #[test]
    fn test_procesar_ip() {
        let ipv4 = crate::version::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 0, 1));
        let expected_result_ipv4 = [
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xff, 0xff, 192, 168, 0, 1,
        ];
        assert_eq!(
            crate::version::VersionMessage::procesar_ip(ipv4),
            expected_result_ipv4
        );
    }
}
