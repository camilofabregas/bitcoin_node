use crate::txn::Txn;

#[derive(Debug, Clone)]
pub struct TxnInfo {
    pub txn: Txn,
    pub date: u32,
    pub txn_type: TxnType,
    pub label: String,
    pub amount: f64,
    pub address: String,
    pub bloque: String,
}

#[derive(Debug, Clone)]
pub enum TxnType {
    Sending,
    Sent,
    Receiving,
    Received,
    Undefined,
}

impl TxnInfo {
    pub fn new(
        txn: Txn,
        txn_type: TxnType,
        label: String,
        amount: f64,
        address: String,
        bloque: String,
    ) -> TxnInfo {
        let date = txn.tx_lock_time;
        TxnInfo {
            txn,
            date,
            txn_type,
            label,
            amount,
            address,
            bloque,
        }
    }
    /// Obtiene el monto, diferenciando el signo segun el
    /// tipo de transaccion (Sending o Receiving)
    pub fn obtain_pending_amount(&self) -> String {
        match self.txn_type {
            crate::txn_info::TxnType::Sending | crate::txn_info::TxnType::Sent => {
                format!("-{:.8} BTC", &self.amount)
            }
            crate::txn_info::TxnType::Receiving | crate::txn_info::TxnType::Received => {
                format!("{:.8} BTC", &self.amount)
            }
            _ => "".to_owned(),
        }
    }
}
