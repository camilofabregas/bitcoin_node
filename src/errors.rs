use std::collections::HashMap;
use std::net::TcpStream;
use std::string::FromUtf8Error;
use std::sync::mpsc::SendError;
use std::sync::{mpsc::Receiver, MutexGuard};

use crate::block_header::BlockHeader;
use crate::gui_events::GuiEvent;
use crate::inv::Inv;
use crate::txn::Txn;
use crate::wallet_events::WalletEvent;

type TrxServer = Vec<(String, Txn)>;

#[derive(Debug, PartialEq, Clone)]
pub enum RustifyError {
    NotFound,
    NoHandleado,
    TryFromSliceError,
    CantThreads,
    Utf8Error,
    EofInesperado,
    TimeConversionError,
    CompactSizeNegative,
    ChannelSend,
    ErrorAlParsearBloque,
    ParseIntFromStrError,
    MutexPoisonError,
    SendErrorString,
    PipeRoto,
    NoHayConexionesDisponibles,
    ErrorConversionBitcoinAddress,
    ValidacionChecksumB58Invalida,
    SendGui,
    GTKError,
    WalletSinFondosSuficientes,
    CheckInvalidoScript,
    ErrorConversionSecretKey,
    ErrorParseoTxn,
    EnvioEventoWallet,
    EnvioInvNotificar,
    NoHayWalletsGuardadas,
    NoSeEncontroBloquePedidoPorCliente,
    NoSeEncontroTransaccionPedidaPorCliente,
    ElNodoNoEncuentraBloquePedido,
    ElNodoNoEncuentraTransaccionPedida,
}

impl From<std::io::Error> for RustifyError {
    fn from(value: std::io::Error) -> Self {
        match value.kind() {
            std::io::ErrorKind::NotFound => RustifyError::NotFound,
            std::io::ErrorKind::UnexpectedEof => RustifyError::EofInesperado,
            _ => {
                let now = chrono::Local::now();
                let timestamp = now.format("%Y-%m-%d %H:%M:%S").to_string();
                println!(
                    "[{}] [CONNEXION] Se obtiene el error inestable {:?} de la std::io",
                    timestamp,
                    value.kind()
                );
                RustifyError::NoHandleado
            }
        }
    }
}

impl From<std::array::TryFromSliceError> for RustifyError {
    fn from(_value: std::array::TryFromSliceError) -> Self {
        RustifyError::TryFromSliceError
    }
}

impl From<std::str::Utf8Error> for RustifyError {
    fn from(_value: std::str::Utf8Error) -> Self {
        RustifyError::Utf8Error
    }
}

impl From<std::sync::mpsc::SendError<BlockHeader>> for RustifyError {
    fn from(_value: std::sync::mpsc::SendError<BlockHeader>) -> Self {
        RustifyError::ChannelSend
    }
}

impl From<std::sync::mpsc::SendError<WalletEvent>> for RustifyError {
    fn from(_value: std::sync::mpsc::SendError<WalletEvent>) -> Self {
        RustifyError::EnvioEventoWallet
    }
}

impl From<std::sync::mpsc::SendError<Inv>> for RustifyError {
    fn from(_value: std::sync::mpsc::SendError<Inv>) -> Self {
        RustifyError::EnvioInvNotificar
    }
}

impl From<FromUtf8Error> for RustifyError {
    fn from(_value: FromUtf8Error) -> Self {
        RustifyError::Utf8Error
    }
}

impl From<std::num::ParseIntError> for RustifyError {
    fn from(_value: std::num::ParseIntError) -> Self {
        RustifyError::ParseIntFromStrError
    }
}

impl From<std::sync::PoisonError<MutexGuard<'_, Receiver<BlockHeader>>>> for RustifyError {
    fn from(_value: std::sync::PoisonError<MutexGuard<'_, Receiver<BlockHeader>>>) -> Self {
        RustifyError::MutexPoisonError
    }
}

impl From<std::sync::PoisonError<MutexGuard<'_, HashMap<String, TcpStream>>>> for RustifyError {
    fn from(_value: std::sync::PoisonError<MutexGuard<'_, HashMap<String, TcpStream>>>) -> Self {
        RustifyError::MutexPoisonError
    }
}

impl From<std::sync::PoisonError<MutexGuard<'_, Vec<Inv>>>> for RustifyError {
    fn from(_value: std::sync::PoisonError<MutexGuard<'_, Vec<Inv>>>) -> Self {
        RustifyError::MutexPoisonError
    }
}

impl From<std::sync::PoisonError<MutexGuard<'_, TrxServer>>> for RustifyError {
    fn from(_value: std::sync::PoisonError<MutexGuard<'_, TrxServer>>) -> Self {
        RustifyError::MutexPoisonError
    }
}

impl From<std::sync::PoisonError<MutexGuard<'_, bool>>> for RustifyError {
    fn from(_value: std::sync::PoisonError<MutexGuard<'_, bool>>) -> Self {
        RustifyError::MutexPoisonError
    }
}

impl From<SendError<String>> for RustifyError {
    fn from(_value: SendError<String>) -> Self {
        RustifyError::SendErrorString
    }
}

impl From<bs58::decode::Error> for RustifyError {
    fn from(_value: bs58::decode::Error) -> Self {
        RustifyError::ErrorConversionBitcoinAddress
    }
}

impl From<SendError<GuiEvent>> for RustifyError {
    fn from(_value: SendError<GuiEvent>) -> Self {
        RustifyError::SendGui
    }
}

impl From<std::sync::PoisonError<MutexGuard<'_, Vec<BlockHeader>>>> for RustifyError {
    fn from(_value: std::sync::PoisonError<MutexGuard<'_, Vec<BlockHeader>>>) -> Self {
        RustifyError::MutexPoisonError
    }
}

impl From<std::sync::PoisonError<MutexGuard<'_, HashMap<Vec<u8>, usize>>>> for RustifyError {
    fn from(_value: std::sync::PoisonError<MutexGuard<'_, HashMap<Vec<u8>, usize>>>) -> Self {
        RustifyError::MutexPoisonError
    }
}

/// Catchea los errores, si los hay, en funciones que no retornan nada en su Ok()
pub fn catch(action: RustifyError) {
    println!("FATAL ERROR: {}", obtener_mensaje_personalizado(action));
}

/// Matchea los tipos de errores con un mensaje personalizado a mostrar en pantalla
pub fn obtener_mensaje_personalizado(tipo: RustifyError) -> String {
    let mensaje = match tipo {
        RustifyError::NotFound => "Not Found IO Error",
        RustifyError::NoHandleado => "RustifyError desconocido",
        RustifyError::TryFromSliceError => "No se pudieron convertir los bytes al dato deseado",
        RustifyError::CantThreads => "Cantidad de threads invalida, debe ser mayor a 0",
        RustifyError::Utf8Error => "No se pudieron convertir los bytes a string",
        RustifyError::EofInesperado => "Hubo un error inesperado de EOF",
        RustifyError::TimeConversionError => "No se pudo convertir horario a Unix Time",
        RustifyError::CompactSizeNegative => {
            "La conversión del compactsize dio error, por ser negativo"
        }
        RustifyError::ChannelSend => "No se pudo enviar el header por el channel al thread",
        RustifyError::ErrorAlParsearBloque => {
            "Ha ocurrido un error al realizar el parseo de un bloque"
        }
        RustifyError::ParseIntFromStrError => "No se pudo convertir la string a entero",
        RustifyError::MutexPoisonError => "Poison en el Mutex de la Threadpool/vector de headers",
        RustifyError::SendErrorString => "No se puede enviar ese string",
        RustifyError::PipeRoto => "El pipe de la conexión TCPStream se ha cerrado inesperadamente",
        RustifyError::ErrorConversionBitcoinAddress => "Ocurrió un error al convertir una bitcoin address",
        RustifyError::ValidacionChecksumB58Invalida => {
            "La validación del checksum del bitcoin adress ha fallado"
        }
        RustifyError::SendGui => "No se pudo enviar el mensaje del evento a la interfaz gráfica.",
        RustifyError::GTKError => "Ocurrió un error de GTK",
        RustifyError::WalletSinFondosSuficientes => "La billetera no posee fondos suficientes",
        RustifyError::CheckInvalidoScript => "Ocurrió un error al validar (OP_EQUAL_VERIFY) en el Signature Script",
        RustifyError::ErrorConversionSecretKey => "Error al convertir Secret Key, verificar que se haya colocado private key en formato hex 64 digitos",
        RustifyError::ErrorParseoTxn => "Error al parsear la transaccion modificada al tipo de dato Message, en el proceso de firma",
        RustifyError::EnvioEventoWallet => "Error al enviar evento a la wallet",
        RustifyError::NoHayWalletsGuardadas => "No hay wallets guardados en disco.",
        RustifyError::NoHayConexionesDisponibles => "Para la DNS especificada, no hay conexiones disponibles",
        RustifyError::NoSeEncontroBloquePedidoPorCliente => "No se encontró el bloque solicitado por el nodo cliente",
        RustifyError::ElNodoNoEncuentraBloquePedido => "El nodo no tiene al bloque solicitado",
        RustifyError::NoSeEncontroTransaccionPedidaPorCliente => "No se encontró la transaccion solicitada por el nodo cliente",
        RustifyError::ElNodoNoEncuentraTransaccionPedida => "El nodo no tiene la transaccion solicitada",
        RustifyError::EnvioInvNotificar => "Error al enviar inv desde el listener al servidor",
    };
    mensaje.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_obtener_mensaje_personalizado() {
        // Test for ErrorDeConexion
        let rustify_error = RustifyError::WalletSinFondosSuficientes;
        let expected_mensaje = "La billetera no posee fondos suficientes".to_string();
        let mensaje = obtener_mensaje_personalizado(rustify_error);
        assert_eq!(mensaje, expected_mensaje);

        // Test for ErrorNoHandleado
        let rustify_error = RustifyError::NoHandleado;
        let expected_mensaje = "RustifyError desconocido".to_string();
        let mensaje = obtener_mensaje_personalizado(rustify_error);
        assert_eq!(mensaje, expected_mensaje);
    }
}
