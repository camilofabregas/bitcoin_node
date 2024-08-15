# 23C1-Rustify-11
Repo for Rust Taller De Programacion 1 FIUBA

## Ejecución
La ejecución del programa es mediante el comando **cargo run -- node.config**, donde *node.config* es la ruta al archivo de configuración.

## Archivo de configuración
El archivo **node.config** contiene los siguientes campos configurables:
- **address:** dirección IP o DNS para conectarse al nodo remoto (seed.testnet.bitcoin.sprovoost.nl:18333 o 192.168.X.XX:18333).
- **server_address** dirección IP o DNS para comportamientos del servidor.
- **timeout_secs 5:** tiempo en segundos en el que se intentará establecer la conexión con el nodo remoto.
- **version:** versión de protocolo que utilizará el nodo.
- **node_network_limited:** servicios soportados por el nodo (0x0400 = node_network_limited).
- **node_network:** servicios soportados por el nodo remoto (0x01 = full node).
- **user_agent_rustify:** user agent customizado del nodo.
- **headers_path:** ruta al archivo de headers descargados.
- **blocks_path:** ruta a la carpeta de bloques descargados.
- **height_bloque_inicial:** altura del primer bloque de la blockchain local.
- **timestamp_bloque_inicial:** timestamp del primer bloque de la blockchain local.
- **cant_threads:** número de threads a utilizar en multi-threading (descarga de bloques).
- **cant_blocks_por_inv:** número de bloques a descargar por cada mensaje *inv*.
- **print_logger:** si es *true*, además de guardar los mensajes en el log, los imprime por pantalla.
- **wallets_path:** ruta a la carpeta que contiene las wallets guardadas.
- **cant_retries:** es la cantidad de retries que realiza el programa para conectarse a un nodo.
- **server_mode:** si es *true* se genera el proceso correspondiente al servidor.
- **cant_max_txn_memoria:** valor que define cuantas transacciones se guardan en memoria en el servidor.
