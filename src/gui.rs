use crate::block_validation::generar_merkle_root_con_merkle_proof;
use crate::block_validation::merkle_proof;
use crate::config::Config;
use crate::errors::RustifyError;
use crate::gui_events::actualizar_gui;
use crate::gui_events::GuiEvent;
use crate::serialized_block::SerializedBlock;
use crate::wallet_events::WalletEvent;
use bitcoin_hashes::sha256d;
use bitcoin_hashes::Hash;
use gtk::prelude::*;
use std::fs::File;
use std::io::Read;
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::thread;

const RECOMMENDED_FEE: f64 = 0.0001;

/// Inicia la interfaz gráfica del programa.
/// Corre en un thread separado para que se ejecute en paralelo con el resto del programa.
pub fn iniciar_gui(
    recv_gui: gtk::glib::Receiver<GuiEvent>,
    sender_node: Sender<WalletEvent>,
    config: &Config,
) {
    let blocks_path = config.blocks_path.clone();
    thread::spawn(move || -> Result<(), RustifyError> {
        if gtk::init().is_err() {
            println!("Failed to initialize GTK.");
            return Err(RustifyError::GTKError);
        }
        let glade_src = include_str!("gui.glade");
        let builder = gtk::Builder::from_string(glade_src);

        let window: gtk::Window = builder
            .object("main_window")
            .ok_or(RustifyError::GTKError)?;
        window.show_all();

        // Logica de los widgets

        definir_logica_cerrar(&builder, sender_node.clone())?;

        definir_logica_send_transaction(&builder, sender_node.clone())?;

        definir_logica_load_wallet(&builder, sender_node.clone())?;

        definir_logica_dialog_add_wallet(&builder)?;
        definir_logica_button_add_wallet(&builder, sender_node)?;
        definir_logica_clear_add_wallet(&builder)?;

        definir_logica_clear(&builder)?;

        definir_logica_recommended_fee(&builder)?;

        definir_logica_warning_sync_dialog(&builder)?;

        definir_logica_about(&builder)?;

        definir_logica_merkle_proof(&builder, blocks_path)?;

        let window2 = Rc::new(window); // La window a usar en MINIMIZE
        let window3 = window2.clone(); // La window a usar en QUIT
        definir_logica_minimize(&builder, window2)?;
        definir_logica_quit(&builder, window3)?;

        actualizar_gui(recv_gui, &builder)?;

        gtk::main();
        Ok(())
    });
}

/// Setea el comportamiento en la interfaz para cerrar el programa.
fn definir_logica_cerrar(
    builder: &gtk::Builder,
    sender_node: Sender<WalletEvent>,
) -> Result<(), RustifyError> {
    let window: gtk::Window = builder
        .object("main_window")
        .ok_or(RustifyError::GTKError)?;
    window.connect_delete_event(move |_, _| {
        sender_node.send(WalletEvent::Cerrar).unwrap_or_else(|_| {
            println!("Error al enviar el mensaje para cerrar por el channel al nodo")
        });
        gtk::main_quit();
        Inhibit(false)
    });
    Ok(())
}

/// Setea el comportamiento en la interfaz para el boton Send Transaction.
fn definir_logica_send_transaction(
    builder: &gtk::Builder,
    sender_node: Sender<WalletEvent>,
) -> Result<(), RustifyError> {
    let entry_pay_to: gtk::Entry = builder
        .object("entry_pay_to")
        .ok_or(RustifyError::GTKError)?;
    let entry_label: gtk::Entry = builder
        .object("entry_label")
        .ok_or(RustifyError::GTKError)?;
    let spin_button_amount: gtk::SpinButton = builder
        .object("spin_button_amount")
        .ok_or(RustifyError::GTKError)?;
    let spin_button_fee: gtk::SpinButton = builder
        .object("spin_button_fee")
        .ok_or(RustifyError::GTKError)?;
    let combo_box_wallets: gtk::ComboBoxText = builder
        .object("combo_box_wallets")
        .ok_or(RustifyError::GTKError)?;
    let stack: gtk::Stack = builder.object("stack").ok_or(RustifyError::GTKError)?;
    let fixed_overview: gtk::Fixed = builder
        .object("fixed_overview")
        .ok_or(RustifyError::GTKError)?;
    let button_clear_all: gtk::Button = builder
        .object("button_clear_all")
        .ok_or(RustifyError::GTKError)?;
    let sent_transaction_dialog: gtk::MessageDialog = builder
        .object("sent_transaction_dialog")
        .ok_or(RustifyError::GTKError)?;

    let button_send: gtk::Button = builder
        .object("button_send")
        .ok_or(RustifyError::GTKError)?;
    button_send.connect_clicked(move |_| {
        sent_transaction_dialog.run();
        sent_transaction_dialog.hide();
        let alias = combo_box_wallets.active_text().unwrap().to_string();
        let address = entry_pay_to.text().to_string();
        let label = entry_label.text().to_string();
        let amount = spin_button_amount.value();
        let fee = spin_button_fee.value();
        sender_node
            .send(WalletEvent::RealizarTransferencia(
                alias, amount, label, address, fee,
            ))
            .unwrap_or_else(|_| {
                println!("Error al enviar la transferencia por el channel a la wallet")
            });
        button_clear_all.activate(); // Limpio los campos
        stack.set_visible_child(&fixed_overview); // Vuelvo al menu principal (Overview)
    });
    Ok(())
}

/// Setea el comportamiento para cargar una wallet desde el ComboBoxText.
fn definir_logica_load_wallet(
    builder: &gtk::Builder,
    sender_node: Sender<WalletEvent>,
) -> Result<(), RustifyError> {
    let fixed_send: gtk::Fixed = builder.object("fixed_send").ok_or(RustifyError::GTKError)?;
    let fixed_transactions: gtk::Fixed = builder
        .object("fixed_transactions")
        .ok_or(RustifyError::GTKError)?;
    let combo_box_wallets: gtk::ComboBoxText = builder
        .object("combo_box_wallets")
        .ok_or(RustifyError::GTKError)?;

    combo_box_wallets.connect_changed(move |combo_box_wallets| {
        fixed_send.set_sensitive(true);
        fixed_transactions.set_sensitive(true);
        sender_node
            .send(WalletEvent::CargarWallet(
                combo_box_wallets.active_text().unwrap().to_string(),
            ))
            .unwrap_or_else(|_| {
                println!("Error al enviar el alias por el channel a la wallet para cargarla")
            });
    });
    Ok(())
}

/// Setea el comportamiento para abrir el dialog de Add Wallet.
fn definir_logica_dialog_add_wallet(builder: &gtk::Builder) -> Result<(), RustifyError> {
    let add_wallet_dialog: gtk::Dialog = builder
        .object("add_wallet_dialog")
        .ok_or(RustifyError::GTKError)?;

    let item_add_new_wallet: gtk::MenuItem = builder
        .object("item_add_new_wallet")
        .ok_or(RustifyError::GTKError)?;
    item_add_new_wallet.connect_activate(move |_| {
        add_wallet_dialog.run();
        add_wallet_dialog.hide();
    });
    Ok(())
}

/// Setea el comportamiento para el boton "Add" de Add Wallet.
fn definir_logica_button_add_wallet(
    builder: &gtk::Builder,
    sender_node: Sender<WalletEvent>,
) -> Result<(), RustifyError> {
    let entry_private_key: gtk::Entry = builder
        .object("entry_private_key")
        .ok_or(RustifyError::GTKError)?;
    let entry_public_key: gtk::Entry = builder
        .object("entry_public_key")
        .ok_or(RustifyError::GTKError)?;
    let entry_alias: gtk::Entry = builder
        .object("entry_alias")
        .ok_or(RustifyError::GTKError)?;
    let add_wallet_dialog: gtk::Dialog = builder
        .object("add_wallet_dialog")
        .ok_or(RustifyError::GTKError)?;
    let button_clear_all_wallet: gtk::Button = builder
        .object("button_clear_all_wallet")
        .ok_or(RustifyError::GTKError)?;
    let combo_box_wallets: gtk::ComboBoxText = builder
        .object("combo_box_wallets")
        .ok_or(RustifyError::GTKError)?;

    let button_add_wallet: gtk::Button = builder
        .object("button_add_wallet")
        .ok_or(RustifyError::GTKError)?;
    button_add_wallet.connect_clicked(move |_| {
        let private_key = entry_private_key.text().to_string();
        let public_key = entry_public_key.text().to_string();
        let alias = entry_alias.text().to_string();
        combo_box_wallets.prepend_text(&alias);
        sender_node
            .send(WalletEvent::AgregarWallet(private_key, public_key, alias))
            .unwrap_or_else(|_| {
                println!("Error al enviar los datos de la wallet por el channel al nodo")
            });
        add_wallet_dialog.hide(); // Cierro el dialog
        button_clear_all_wallet.activate(); // Limpio los campos
        combo_box_wallets.set_active(Some(0));
    });
    Ok(())
}

/// Setea el comportamiento para el boton "Clear All" de Add Wallet.
fn definir_logica_clear_add_wallet(builder: &gtk::Builder) -> Result<(), RustifyError> {
    let entry_private_key: gtk::Entry = builder
        .object("entry_private_key")
        .ok_or(RustifyError::GTKError)?;
    let entry_public_key: gtk::Entry = builder
        .object("entry_public_key")
        .ok_or(RustifyError::GTKError)?;
    let entry_alias: gtk::Entry = builder
        .object("entry_alias")
        .ok_or(RustifyError::GTKError)?;

    let button_clear_all_wallet: gtk::Button = builder
        .object("button_clear_all_wallet")
        .ok_or(RustifyError::GTKError)?;
    button_clear_all_wallet.connect_clicked(move |_| {
        entry_private_key.set_text("");
        entry_public_key.set_text("");
        entry_alias.set_text("");
    });
    Ok(())
}

/// Setea el comportamiento en la interfaz para los botones Clear y Clear All.
fn definir_logica_clear(builder: &gtk::Builder) -> Result<(), RustifyError> {
    let entry_pay_to: gtk::Entry = builder
        .object("entry_pay_to")
        .ok_or(RustifyError::GTKError)?;
    let entry_pay_to_2 = Rc::new(entry_pay_to); // La entry a limpiar en CLEAN
    let entry_pay_to_3 = entry_pay_to_2.clone(); // La entry a limpiar en CLEAN ALL

    // Clear
    let button_clear_pay_to: gtk::Button = builder
        .object("button_clear_pay_to")
        .ok_or(RustifyError::GTKError)?;
    button_clear_pay_to.connect_clicked(move |_| {
        entry_pay_to_2.set_text("");
    });

    // Clear All
    let entry_label: gtk::Entry = builder
        .object("entry_label")
        .ok_or(RustifyError::GTKError)?;
    let spin_button_amount: gtk::SpinButton = builder
        .object("spin_button_amount")
        .ok_or(RustifyError::GTKError)?;
    let spin_button_fee: gtk::SpinButton = builder
        .object("spin_button_fee")
        .ok_or(RustifyError::GTKError)?;
    let button_clear_all: gtk::Button = builder
        .object("button_clear_all")
        .ok_or(RustifyError::GTKError)?;
    button_clear_all.connect_clicked(move |_| {
        entry_pay_to_3.set_text("");
        entry_label.set_text("");
        spin_button_amount.set_value(0.0);
        spin_button_fee.set_value(0.0);
    });
    Ok(())
}

fn definir_logica_recommended_fee(builder: &gtk::Builder) -> Result<(), RustifyError> {
    let button_fill_fee: gtk::Button = builder
        .object("button_fill_fee")
        .ok_or(RustifyError::GTKError)?;
    let spin_button_fee: gtk::SpinButton = builder
        .object("spin_button_fee")
        .ok_or(RustifyError::GTKError)?;

    button_fill_fee.connect_clicked(move |_| {
        spin_button_fee.set_value(RECOMMENDED_FEE);
    });
    Ok(())
}

/// Setea el comportamiento en la interfaz para correr el MessageDialog warning_sync_dialog.
fn definir_logica_warning_sync_dialog(builder: &gtk::Builder) -> Result<(), RustifyError> {
    let button_balances: gtk::Button = builder
        .object("button_balances")
        .ok_or(RustifyError::GTKError)?;
    let button_recent_txs: gtk::Button = builder
        .object("button_recent_txs")
        .ok_or(RustifyError::GTKError)?;
    let warning_sync_dialog: gtk::MessageDialog = builder
        .object("warning_sync_dialog")
        .ok_or(RustifyError::GTKError)?;
    let warning_sync_dialog2 = Rc::new(warning_sync_dialog); // MessageDialog para la primera closure.
    let warning_sync_dialog3 = warning_sync_dialog2.clone(); // MessageDialog para la primera closure.
    button_balances.connect_clicked(move |_| {
        warning_sync_dialog2.run();
        warning_sync_dialog2.hide();
    });
    button_recent_txs.connect_clicked(move |_| {
        warning_sync_dialog3.run();
        warning_sync_dialog3.hide();
    });
    Ok(())
}

/// Setea el comportamiento en la interfaz para correr el AboutDialog.
fn definir_logica_about(builder: &gtk::Builder) -> Result<(), RustifyError> {
    let item_about: gtk::MenuItem = builder.object("item_about").ok_or(RustifyError::GTKError)?;
    let about_dialog: gtk::AboutDialog = builder
        .object("about_dialog")
        .ok_or(RustifyError::GTKError)?;
    item_about.connect_activate(move |_| {
        about_dialog.run();
        about_dialog.hide();
    });
    Ok(())
}

/// Setea el comportamiento en la interfaz para pedir la Merkle Proof de una transacción enviada.
fn definir_logica_merkle_proof(
    builder: &gtk::Builder,
    block_path: String,
) -> Result<(), RustifyError> {
    let cr_tree_view_transactions: gtk::CellRendererToggle = builder
        .object("cr_tree_view_transactions")
        .ok_or(RustifyError::GTKError)?;
    let list_store_transactions: gtk::ListStore = builder
        .object("list_store_transactions")
        .ok_or(RustifyError::GTKError)?;
    let merkle_proof_dialog: gtk::MessageDialog = builder
        .object("merkle_proof_dialog")
        .ok_or(RustifyError::GTKError)?;

    cr_tree_view_transactions.connect_toggled(move |_, path| {
        let iter = list_store_transactions
            .iter_from_string(path.to_str().as_str())
            .expect("Error al obtener el iter del list_store_transactions");
        let txn_hex_bytes: String = list_store_transactions
            .value(&iter, 6)
            .get()
            .expect("Error al obtener la txn del list_store_transactions");
        let bloque: String = list_store_transactions
            .value(&iter, 7)
            .get()
            .expect("Error al obtener el bloque del list_store_transactions");

        if bloque != *"-" {
            let txn_bytes = (0..txn_hex_bytes.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&txn_hex_bytes[i..i + 2], 16))
                .collect::<Result<Vec<u8>, _>>()
                .expect("Error al convertir la txn hexa a bytes");
            let txid = sha256d::Hash::hash(&txn_bytes).to_byte_array().to_vec();
            let txid_hex: String = txid.iter().map(|b| format!("{:02x}", b) + "").collect();

            let mut archivo_bloque = File::options()
                .read(true)
                .write(true)
                .create(false)
                .open(format!("{}/{}.txt", block_path, &bloque))
                .expect("Error al obtener el archivo del bloque");
            let mut buffer = vec![];
            let _lectura = archivo_bloque.read_to_end(&mut buffer);
            let block = SerializedBlock::from_bytes(&buffer)
                .expect("Error al generar el bloque a partir de los bytes");

            let merkle_proof = merkle_proof(txid, &block);
            let merkle_root = generar_merkle_root_con_merkle_proof(&merkle_proof);

            let merkle_root_hex: String = merkle_root
                .iter()
                .map(|b| format!("{:02x}", b) + "")
                .collect();

            let mut merkle_proof_hex = "".to_string();
            for tuple in merkle_proof {
                let hash_hex: String = tuple.0.iter().map(|b| format!("{:02x}", b) + "").collect();
                merkle_proof_hex += &(hash_hex + " ," + tuple.1 + "\n");
            }

            let merkle_proof_string = format!(
                "Transaction: {}\n\nBlock: {}\n\nMerkle Proof: {}\nMerkle Root: {}\n",
                txid_hex, bloque, merkle_proof_hex, merkle_root_hex
            );
            merkle_proof_dialog.set_secondary_text(Some(merkle_proof_string.as_str()));
            merkle_proof_dialog.run();
            merkle_proof_dialog.hide();
        }
    });
    Ok(())
}

/// Setea el comportamiento en la interfaz para minimizarla.
fn definir_logica_minimize(
    builder: &gtk::Builder,
    window: Rc<gtk::Window>,
) -> Result<(), RustifyError> {
    let item_minimize: gtk::MenuItem = builder
        .object("item_minimize")
        .ok_or(RustifyError::GTKError)?;
    item_minimize.connect_activate(move |_| {
        window.iconify();
    });
    Ok(())
}

/// Setea el comportamiento en la interfaz para cerrarla.
fn definir_logica_quit(
    builder: &gtk::Builder,
    window: Rc<gtk::Window>,
) -> Result<(), RustifyError> {
    let item_quit: gtk::MenuItem = builder.object("item_quit").ok_or(RustifyError::GTKError)?;
    item_quit.connect_activate(move |_| {
        window.close();
    });
    Ok(())
}
