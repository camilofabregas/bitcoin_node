use std::fs;

#[derive(Debug, Clone)]
pub struct Config {
    pub address: String,
    pub server_address: String,
    pub timeout_secs: u64,
    pub version: i32,
    pub node_network_limited: u64,
    pub node_network: u64,
    pub user_agent_rustify: String,
    pub headers_path: String,
    pub blocks_path: String,
    pub height_bloque_inicial: usize,
    pub timestamp_bloque_inicial: u32,
    pub cant_threads: usize,
    pub cant_blocks_por_inv: u32,
    pub print_logger: bool,
    pub wallets_path: String,
    pub cant_retries: usize,
    pub server_mode: bool,
    pub cant_max_txn_memoria: usize,
}

impl Config {
    pub fn new(config_file_path: &str) -> Result<Self, String> {
        let contents = fs::read_to_string(config_file_path)
            .map_err(|e| format!("Error reading config file: {}", e))?;
        let mut config = Config {
            address: "".to_string(),
            server_address: "".to_string(),
            timeout_secs: 0,
            version: 0,
            node_network_limited: 0,
            node_network: 0,
            user_agent_rustify: "".to_string(),
            headers_path: "".to_string(),
            blocks_path: "".to_string(),
            height_bloque_inicial: 0,
            timestamp_bloque_inicial: 0,
            cant_threads: 0,
            cant_blocks_por_inv: 0,
            print_logger: true,
            wallets_path: "".to_string(),
            cant_retries: 0,
            server_mode: true,
            cant_max_txn_memoria: 0,
        };
        for line in contents.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() != 2 {
                return Err(format!("Invalid config line: {}", line));
            }
            match parts[0] {
                "address" => config.address = parts[1].to_string(),
                "server_address" => config.server_address = parts[1].to_string(),
                "timeout_secs" => {
                    config.timeout_secs = parts[1]
                        .parse()
                        .map_err(|e| format!("Error parsing version: {}", e))?
                }
                "version" => {
                    config.version = parts[1]
                        .parse()
                        .map_err(|e| format!("Error parsing version: {}", e))?
                }
                "node_network_limited" => {
                    config.node_network_limited = u64::from_str_radix(&parts[1][2..], 16)
                        .map_err(|e| format!("Error parsing partial node: {}", e))?
                }
                "node_network" => {
                    config.node_network = u64::from_str_radix(&parts[1][2..], 16)
                        .map_err(|e| format!("Error parsing node network: {}", e))?
                }
                "user_agent_rustify" => config.user_agent_rustify = parts[1].to_string(),
                "headers_path" => config.headers_path = parts[1].to_string(),
                "blocks_path" => config.blocks_path = parts[1].to_string(),
                "height_bloque_inicial" => {
                    config.height_bloque_inicial = parts[1]
                        .parse()
                        .map_err(|e| format!("Error parsing height_bloque_inicial: {}", e))?
                }
                "timestamp_bloque_inicial" => {
                    config.timestamp_bloque_inicial = parts[1]
                        .parse()
                        .map_err(|e| format!("Error parsing timestamp_bloque_inicial: {}", e))?
                }
                "cant_threads" => {
                    config.cant_threads = parts[1]
                        .parse()
                        .map_err(|e| format!("Error parsing cant_threads: {}", e))?
                }
                "cant_blocks_por_inv" => {
                    config.cant_blocks_por_inv = parts[1]
                        .parse()
                        .map_err(|e| format!("Error parsing cant_blocks_por_inv: {}", e))?
                }
                "print_logger" => {
                    config.print_logger = parts[1]
                        .parse()
                        .map_err(|e| format!("Error parsing init_logger: {}", e))?
                }
                "wallets_path" => config.wallets_path = parts[1].to_string(),
                "cant_retries" => {
                    config.cant_retries = parts[1]
                        .parse()
                        .map_err(|e| format!("Error parsing cant_retries: {}", e))?
                }
                "server_mode" => {
                    config.server_mode = parts[1]
                        .parse()
                        .map_err(|e| format!("Error parsing init_logger: {}", e))?
                }
                "cant_max_txn_memoria" => {
                    config.cant_max_txn_memoria = parts[1]
                        .parse()
                        .map_err(|e| format!("Error parsing cant_max_txn_memoria: {}", e))?
                }
                _ => return Err(format!("Unknown config parameter: {}", parts[0])),
            }
        }
        Ok(config)
    }

    /// Carga el archivo de configuración en una estructura Config.
    /// Esta estructura es pasada por parámetro donde se requiera un valor configurable.
    pub fn load_config(args: &[String]) -> Result<Config, String> {
        if args.len() != 2 {
            return Err("Usage: cargo run -- path/to/nodo.config".to_string());
        }

        let config_file_path = &args[1];
        let config = Config::new(config_file_path)?;

        Ok(config)
    }
}
