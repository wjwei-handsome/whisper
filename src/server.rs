use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    username: String,
    content: String,
    // TODO: should add a timestamp field??
}

type SharedClients = Arc<Mutex<Vec<tokio::net::tcp::OwnedWriteHalf>>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // start a new server in 8080
    // TODO: should add a port argument
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    println!("Server is running on 0.0.0.0:8080");

    // share connect by Arc and tokio::Mutex
    let clients: SharedClients = Arc::new(Mutex::new(Vec::new()));

    loop {
        // accept new connection
        let (socket, addr) = listener.accept().await?;
        println!("New connection from {}", addr);

        // clone the clients
        let clients = Arc::clone(&clients);

        // start a new task to handle the connection
        tokio::spawn(handle_connection(socket, clients));
    }
}

async fn handle_connection(socket: TcpStream, clients: SharedClients) -> anyhow::Result<()> {
    // split the socket into reader and writer
    let (reader, writer) = socket.into_split();
    let mut reader = BufReader::new(reader);

    // when user join the chat, read the username
    let mut username = String::new();
    reader.read_line(&mut username).await?;
    let username = username.trim().to_string();
    println!("User '{}' joined the chat", username);
    // TODO: some welcome message???

    let welcome_message = Message {
        username: "Server".to_string(),
        content: format!("{} joined the chat", username),
    };
    let welcome_json = serde_json::to_string(&welcome_message)?;
    let mut writer = writer;
    writer.write_all(welcome_json.as_bytes()).await?;
    writer.write_all(b"\n").await?;

    // lock the clients and add the new client
    {
        let mut clients = clients.lock().await;
        clients.push(writer);
    }

    // handle the client messages
    handle_client_messages(reader, username, clients).await?;

    Ok(())
}

async fn handle_client_messages(
    mut reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    username: String,
    clients: SharedClients,
) -> anyhow::Result<()> {
    let mut buffer = String::new();

    loop {
        buffer.clear();
        let bytes_read = reader.read_line(&mut buffer).await?;
        // when the client disconnect
        if bytes_read == 0 {
            println!("User '{}' disconnected", username);
            break;
        }

        let content = buffer.trim().to_string();
        let message = Message {
            username: username.clone(),
            content,
        };

        // broadcast the message to all clients
        broadcast_message(&message, &clients).await?;
    }

    Ok(())
}

async fn broadcast_message(message: &Message, clients: &SharedClients) -> anyhow::Result<()> {
    let json_message = serde_json::to_string(message)?;
    let mut clients_to_remove = Vec::new();

    {
        let mut clients = clients.lock().await;

        for (i, writer) in clients.iter_mut().enumerate() {
            if let Err(e) = writer.write_all(json_message.as_bytes()).await {
                eprintln!("Failed to send message to a client: {}", e);
                clients_to_remove.push(i);
            } else {
                writer.write_all(b"\n").await?;
            }
        }

        // remove the fucking disconnected clients
        for &index in clients_to_remove.iter().rev() {
            clients.remove(index);
        }
    }

    Ok(())
}
