use crate::block::block_download;
use crate::block_header::BlockHeader;
use crate::config::Config;
use crate::errors::RustifyError;
use crate::logger::{log, log_with_parameters, Action, Lvl};
use crate::node::{conectar, handshake};
use std::sync::mpsc::Sender;
use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
};

/// Estructura que contiene los workers (threads) para paralelizar la descarga de bloques.
/// También tiene un channel para poder enviarle los headers a los threads para descargar los bloques asociados.
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<BlockHeader>,
}

impl ThreadPool {
    /// Constructor de la ThreadPool, indicando cantidad de threads y los archivos de config y log.
    pub fn build(config: &Config, log_sender: &Sender<String>) -> Result<ThreadPool, RustifyError> {
        if config.cant_threads == 0 {
            return Err(RustifyError::CantThreads);
        }

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(config.cant_threads);

        for id in 0..config.cant_threads {
            workers.push(Worker::build(
                id,
                Arc::clone(&receiver),
                config,
                log_sender,
            )?);
        }

        log(
            Lvl::Info(Action::THREADPOOL),
            "Threads creados y Threadpool inicializada",
            log_sender,
        );

        Ok(ThreadPool { workers, sender })
    }

    /// Descarga paralelizada de bloques. Recibe el vector de headers para descargar los bloques.
    /// Cada thread recibe por el channel un header para descargar el bloque asociado.
    pub fn download_blocks(
        self,
        headers: Vec<BlockHeader>,
        logger_sender: &Sender<String>,
    ) -> Result<(), RustifyError> {
        for header in headers {
            self.sender.send(header)?;
        }

        self.wait_for_threads(logger_sender)?;

        Ok(())
    }

    /// Apaga los threads al finalizar la descarga de bloques.
    /// Desconecta al sender para que los threads salgan del loop y finalicen su ejecución.
    /// Finalmente hace el join de los threads, para cada worker.
    fn wait_for_threads(self, logger_sender: &Sender<String>) -> Result<(), RustifyError> {
        drop(self.sender);
        for worker in self.workers {
            match worker.thread.join() {
                Ok(_) => {}
                Err(_) => {
                    log_with_parameters(
                        Lvl::Error(Action::THREADPOOL),
                        format!("Falla en el worker {}.", worker.id),
                        logger_sender,
                    );
                }
            }
            log_with_parameters(
                Lvl::Info(Action::THREADPOOL),
                format!("Apagando worker {}.", worker.id),
                logger_sender,
            );
        }
        Ok(())
    }
}

/// Estructura que contiene un thread y un ID que lo identifica.
/// Cada worker va a descargar de a un bloque.
struct Worker {
    id: usize,
    thread: thread::JoinHandle<Result<(), RustifyError>>,
}

impl Worker {
    /// Constructor de los workers.
    /// Cada uno se conecta a un nodo y hace un handshake para descargar los bloques.
    /// Una vez que spawnean un thread se quedan esperando a que les lleguen headers por el channel para descargar los bloques.
    fn build(
        id: usize,
        receiver: Arc<Mutex<mpsc::Receiver<BlockHeader>>>,
        config: &Config,
        logger_sender: &Sender<String>,
    ) -> Result<Worker, RustifyError> {
        let mut socket = conectar(config, logger_sender)?;
        handshake(&mut socket, config, logger_sender)?;

        let block_path = config.blocks_path.clone();
        let cant_block_for_inv = config.cant_blocks_por_inv;
        let logger_sender_clone = logger_sender.clone();

        log_with_parameters(
            Lvl::Info(Action::THREADPOOL),
            format!("Worker {:?} conectado y listo para descargar bloques.", id),
            &logger_sender_clone,
        );

        let thread = thread::spawn(move || -> Result<(), RustifyError> {
            loop {
                let mensaje = receiver.lock()?.recv();
                match mensaje {
                    Ok(header) => {
                        let header_bytes: String = header
                            .as_bytes()
                            .iter()
                            .map(|b| format!("{:02x}", b) + "")
                            .collect();
                        log_with_parameters(
                            Lvl::Info(Action::THREADPOOL),
                            format!("Worker {:?} descargando el header {}", id, header_bytes),
                            &logger_sender_clone,
                        );
                        block_download(
                            &mut socket,
                            header,
                            block_path.to_string(),
                            cant_block_for_inv,
                            &logger_sender_clone,
                        )?;
                    }
                    Err(_) => {
                        log_with_parameters(
                            Lvl::Info(Action::THREADPOOL),
                            format!("Worker {:?} desconectado; apagando.", id),
                            &logger_sender_clone,
                        );
                        break;
                    }
                }
            }
            Ok(())
        });

        Ok(Worker { id, thread })
    }
}
