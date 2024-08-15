use crate::account::Account;
use crate::errors::RustifyError;
use crate::{block_header::BlockHeader, txn_info::TxnInfo};
use bitcoin_hashes::{sha256d, Hash};
use chrono::{TimeZone, Utc};
use gtk::prelude::*;

const PROGRESS_BAR_STEP: f64 = 0.2;
const CANT_PEND_TXN: usize = 5;

/// Estructura para matchear los distintos eventos que modifican el estado de la interfaz gráfica.
pub enum GuiEvent {
    ActualizarLabelEstado(String),
    OcultarEstado,
    CargarBloques(Vec<BlockHeader>, u32),
    ActualizarWallet(Account),
    IniciarWallets(Vec<String>),
}

/// Handlea los distintos GuiEvent que llegan por el receiver del channel de la interfaz.
/// Estos GuiEvent son señales que indican cómo debe cambiar el estado de la interfaz.
pub fn actualizar_gui(
    recv_gui: gtk::glib::Receiver<GuiEvent>,
    builder: &gtk::Builder,
) -> Result<(), RustifyError> {
    let progress_bar_state: gtk::ProgressBar = builder
        .object("progress_bar_state")
        .ok_or(RustifyError::GTKError)?;
    let label_state: gtk::Label = builder
        .object("label_state")
        .ok_or(RustifyError::GTKError)?;
    let spinner_state: gtk::Spinner = builder
        .object("spinner_state")
        .ok_or(RustifyError::GTKError)?;
    let list_store_blocks: gtk::ListStore = builder
        .object("list_store_blocks")
        .ok_or(RustifyError::GTKError)?;
    let button_balances: gtk::Button = builder
        .object("button_balances")
        .ok_or(RustifyError::GTKError)?;
    let button_recent_txs: gtk::Button = builder
        .object("button_recent_txs")
        .ok_or(RustifyError::GTKError)?;
    let combo_box_wallets: gtk::ComboBoxText = builder
        .object("combo_box_wallets")
        .ok_or(RustifyError::GTKError)?;
    let list_store_transactions: gtk::ListStore = builder
        .object("list_store_transactions")
        .ok_or(RustifyError::GTKError)?;

    let builder_2 = builder.clone(); // Builder necesario para GuiEvent::ActualizarWallet
    recv_gui.attach(None, move |event| {
        match event {
            // Actualiza la barra de estado (label + progress bar) para mostrar los pasos de sincronizacion del nodo.
            GuiEvent::ActualizarLabelEstado(estado) => {
                progress_bar_state.set_fraction(progress_bar_state.fraction() + PROGRESS_BAR_STEP);
                label_state.set_text(&estado);
            }
            // Oculta todos los widgets de la barra de estado para mostrar que finalizo la sincronizacion del nodo.
            GuiEvent::OcultarEstado => {
                progress_bar_state.hide();
                spinner_state.hide();
                button_balances.hide();
                button_recent_txs.hide();
            }
            // Carga y muestra en la pestaña "Blocks" todos los bloques descargados localmente.
            GuiEvent::CargarBloques(headers, mut indice) => {
                for header in headers {
                    let header_hash = sha256d::Hash::hash(&header.as_bytes()).to_string();
                    let fecha = Utc
                        .timestamp_opt(header.time as i64, 0)
                        .unwrap()
                        .format("%Y-%m-%d %a %H:%M:%S")
                        .to_string();
                    list_store_blocks.insert_with_values(
                        Some(0),
                        &[(0, &indice), (1, &header_hash), (2, &fecha)],
                    );
                    indice += 1;
                }
            }
            // Actualiza balance y transacciones de la wallet activa. Esto ocurre cada vez que se selecciona una wallet, o se recibe o envia dinero.
            GuiEvent::ActualizarWallet(wallet) => {
                list_store_transactions.clear();
                actualizar_gui_balance(&wallet, &builder_2)
                    .unwrap_or_else(|_| println!("Error al actualizar el balance en la interfaz."));
                actualizar_gui_txns(&wallet.sent_txn, "Sent", &builder_2).unwrap_or_else(|_| {
                    println!("Error al actualizar transacciones en la interfaz.")
                });
                actualizar_gui_txns(&wallet.obtain_utxo_info(), "Received", &builder_2)
                    .unwrap_or_else(|_| {
                        println!("Error al actualizar transacciones en la interfaz.")
                    });
                actualizar_gui_pending_txns(&wallet, &builder_2).unwrap_or_else(|_| {
                    println!("Error al actualizar transacciones pendientes en la interfaz.")
                });
            }
            GuiEvent::IniciarWallets(aliases) => {
                for alias in aliases {
                    combo_box_wallets.prepend_text(&alias);
                }
            }
        }
        Continue(true)
    });

    Ok(())
}

/// Actualiza el balance de la cuenta activa.
fn actualizar_gui_balance(wallet: &Account, builder: &gtk::Builder) -> Result<(), RustifyError> {
    let label_available_btc: gtk::Label = builder
        .object("label_available_btc")
        .ok_or(RustifyError::GTKError)?;
    let label_pending_btc: gtk::Label = builder
        .object("label_pending_btc")
        .ok_or(RustifyError::GTKError)?;
    let label_total_btc: gtk::Label = builder
        .object("label_total_btc")
        .ok_or(RustifyError::GTKError)?;

    label_available_btc.set_text(&format!("{:.8} BTC", &wallet.balance));
    label_pending_btc.set_text(&format!("{:.8} BTC", &wallet.pending_balance));
    label_total_btc.set_text(&format!(
        "{:.8} BTC",
        (wallet.balance + wallet.pending_balance)
    ));

    Ok(())
}

/// Actualiza el historial de transacciones de la cuenta activa.
/// Esto incluye las transacciones enviadas (txn_type == "Sent") y las recibidas o UTXO (txn_type == "Received").

fn actualizar_gui_txns(
    txns: &Vec<TxnInfo>,
    txn_type: &str,
    builder: &gtk::Builder,
) -> Result<(), RustifyError> {
    let list_store_transactions: gtk::ListStore = builder
        .object("list_store_transactions")
        .ok_or(RustifyError::GTKError)?;

    for txn_info in txns {
        let fecha = Utc
            .timestamp_opt(txn_info.date as i64, 0)
            .unwrap()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        let amount_str = txn_info.obtain_pending_amount();

        let txn_hex_bytes: String = txn_info
            .txn
            .as_bytes()
            .iter()
            .map(|b| format!("{:02x}", b) + "")
            .collect();

        list_store_transactions.insert_with_values(
            Some(0),
            &[
                (1, &fecha),
                (2, &txn_type),
                (3, &txn_info.label),
                (4, &txn_info.address),
                (5, &amount_str),
                (6, &txn_hex_bytes),
                (7, &txn_info.bloque),
            ],
        );
    }

    Ok(())
}

/// Actualiza el historial de transacciones pendientes.
/// Esta ubicado en la sección "Pending transactions" de la solapa "Overview".
/// Se muestra un máximo de 5 transacciones pendientes, ordenadas por más recientes.
/// De esas 5, 3 corresponderán a "Sending", y 2 a "Receiving"
fn actualizar_gui_pending_txns(
    wallet: &Account,
    builder: &gtk::Builder,
) -> Result<(), RustifyError> {
    let labels_pend_date: Vec<gtk::Label> = vec![
        builder.object("tx1_date").ok_or(RustifyError::GTKError)?,
        builder.object("tx2_date").ok_or(RustifyError::GTKError)?,
        builder.object("tx3_date").ok_or(RustifyError::GTKError)?,
        builder.object("tx4_date").ok_or(RustifyError::GTKError)?,
        builder.object("tx5_date").ok_or(RustifyError::GTKError)?,
    ];
    let labels_pend_amount: Vec<gtk::Label> = vec![
        builder.object("tx1_amount").ok_or(RustifyError::GTKError)?,
        builder.object("tx2_amount").ok_or(RustifyError::GTKError)?,
        builder.object("tx3_amount").ok_or(RustifyError::GTKError)?,
        builder.object("tx4_amount").ok_or(RustifyError::GTKError)?,
        builder.object("tx5_amount").ok_or(RustifyError::GTKError)?,
    ];
    let labels_pend_address: Vec<gtk::Label> = vec![
        builder
            .object("tx1_address")
            .ok_or(RustifyError::GTKError)?,
        builder
            .object("tx2_address")
            .ok_or(RustifyError::GTKError)?,
        builder
            .object("tx3_address")
            .ok_or(RustifyError::GTKError)?,
        builder
            .object("tx4_address")
            .ok_or(RustifyError::GTKError)?,
        builder
            .object("tx5_address")
            .ok_or(RustifyError::GTKError)?,
    ];
    let icons_pend: Vec<gtk::Image> = vec![
        builder.object("icon_tx1").ok_or(RustifyError::GTKError)?,
        builder.object("icon_tx2").ok_or(RustifyError::GTKError)?,
        builder.object("icon_tx3").ok_or(RustifyError::GTKError)?,
        builder.object("icon_tx4").ok_or(RustifyError::GTKError)?,
        builder.object("icon_tx5").ok_or(RustifyError::GTKError)?,
    ];

    let pending_txn = wallet.pending_txn();

    for pend_txn_slot in 0..CANT_PEND_TXN {
        let pending_txn = pending_txn.get(pend_txn_slot);
        match pending_txn {
            Some(txn_info) => {
                let fecha = Utc
                    .timestamp_opt(txn_info.date as i64, 0)
                    .unwrap()
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string();
                let amount_str = txn_info.obtain_pending_amount();
                labels_pend_date[pend_txn_slot].set_text(&fecha);
                labels_pend_date[pend_txn_slot].set_visible(true);
                labels_pend_amount[pend_txn_slot].set_text(&amount_str);
                labels_pend_amount[pend_txn_slot].set_visible(true);
                labels_pend_address[pend_txn_slot].set_text(&txn_info.address);
                labels_pend_address[pend_txn_slot].set_visible(true);
                icons_pend[pend_txn_slot].set_visible(true);

                match txn_info.txn_type {
                    crate::txn_info::TxnType::Sending => {
                        icons_pend[pend_txn_slot].set_from_file(Some("assets/send.png"))
                    }
                    crate::txn_info::TxnType::Receiving => {
                        icons_pend[pend_txn_slot].set_from_file(Some("assets/receive.png"))
                    }
                    _ => {}
                };
            }
            None => {
                labels_pend_date[pend_txn_slot].set_text("");
                labels_pend_date[pend_txn_slot].set_visible(false);
                labels_pend_amount[pend_txn_slot].set_text("");
                labels_pend_amount[pend_txn_slot].set_visible(false);
                labels_pend_address[pend_txn_slot].set_text("");
                labels_pend_address[pend_txn_slot].set_visible(false);
                icons_pend[pend_txn_slot].set_visible(false);
            }
        }
    }

    Ok(())
}
