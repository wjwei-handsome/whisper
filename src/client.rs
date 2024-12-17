use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

#[derive(Serialize, Deserialize)]
struct Message {
    username: String,
    content: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // read username
    print!("Enter your username: ");
    io::stdout().flush()?; // flush the stdout
    let mut username = String::new();
    io::stdin().read_line(&mut username)?;
    let username = username.trim().to_string();

    // start new connection to the server
    let socket = TcpStream::connect("127.0.0.1:8080").await?;
    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader);

    // send username to the server
    writer
        .write_all(format!("{}\n", username).as_bytes())
        .await?;
    // println!("Username sent, waiting for server response...");

    // read server response
    let mut buffer = String::new();
    reader.read_line(&mut buffer).await?;
    let trimmed = buffer.trim();
    if let Ok(message) = serde_json::from_str::<Message>(trimmed) {
        println!("[{}]: {}", message.username, message.content);
    } else {
        println!("Server message: {}", trimmed); // 非 JSON 消息，直接打印
    }

    // main task to read message from server
    let read_task = tokio::spawn(async move {
        let mut buffer = String::new();
        loop {
            buffer.clear();
            let bytes_read = reader.read_line(&mut buffer).await.unwrap();
            if bytes_read == 0 {
                println!("Connection closed by server");
                break;
            }
            let trimmed = buffer.trim();
            if let Ok(message) = serde_json::from_str::<Message>(trimmed) {
                println!("[{}]: {}", message.username, message.content);
            } else {
                println!("Server message: {}", trimmed);
            }
        }
    });

    // main task to write message to server
    let write_task = tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let mut stdin_reader = BufReader::new(stdin);
        let mut input = String::new();
        loop {
            input.clear();
            stdin_reader.read_line(&mut input).await.unwrap();
            let message = Message {
                username: username.clone(),
                content: input.trim().to_string(),
            };
            // let json_message = serde_json::to_string(&message).unwrap();
            writer.write_all(message.content.as_bytes()).await.unwrap();
            writer.write_all(b"\n").await.unwrap();
        }
    });

    // wait for both tasks to finish
    tokio::try_join!(read_task, write_task)?;
    Ok(())
}
