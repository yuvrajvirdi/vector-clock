use serde::{Deserialize, Serialize};
use serde_json::to_string;
use std::env;
use std::fs::OpenOptions;
use std::io::{self, BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, Condvar};
use std::thread;
use chrono::Local;

// vector clock represented by a vector
type VectorClock = [i32; 3];

// server message structure
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ServerMessage {
    sender_id: String,
    clock: VectorClock,
}

// tcp server structure
struct Server {
    id: String,
    clock_index: usize,
    clock: VectorClock,
    listener: TcpListener,
    shutdown_signal: Arc<(Mutex<bool>, Condvar)>,
}

// server implementation
impl Server {
    // binds server to port
    fn new(id: &str, clock_index: usize, port: u16) -> Self {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
        listener.set_nonblocking(true).expect("Failed to set non-blocking");
        let mut clock = [0; 3];
        clock[clock_index] = 1;
        Self {
            id: id.to_string(),
            clock_index,
            clock,
            listener,
            shutdown_signal: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    // increments logical time
    fn increment(&mut self) {
        self.clock[self.clock_index] += 1;
    }

    // updates this clock based on larger time value
    fn update_clock(&mut self, other_clock: &VectorClock) {
        for i in 0..3 {
            self.clock[i] = self.clock[i].max(other_clock[i]);
        }
        self.increment();
    }

    // sends event to other server at other_address
    fn send_event(&mut self, other_address: &str) {
        self.increment();
        let msg = ServerMessage {
            sender_id: self.id.clone(),
            clock: self.clock,
        };
        let mut stream = TcpStream::connect(other_address).unwrap();
        let msg_json = to_string(&msg).unwrap();
        stream.write_all(msg_json.as_bytes()).unwrap();
        let log_msg = format!("{} send an event to {} with clock {:?}", self.id, other_address, self.clock);
        println!("{}", log_msg);
        Server::log_event(&log_msg);
    }

    // handles incoming events
    fn handle_events(&mut self) {
        while !*self.shutdown_signal.0.lock().unwrap() {
            match self.listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0; 1024];
                    let _ = stream.read(&mut buffer).unwrap();
                    let msg: ServerMessage = serde_json::from_slice(&buffer).expect("cannot deserialize message");
                    self.update_clock(&msg.clock);
                    let log_msg = format!("received event from {}, clock is now {:?}", msg.sender_id, self.clock);
                    println!("{}", log_msg);
                    Server::log_event(&log_msg);
                },
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No incoming connection, break the loop to avoid busy waiting
                    break;
                },
                Err(e) => {
                    eprintln!("Error accepting connection: {}", e);
                    continue;
                }
            }
        }
    }

    // event logger
    fn log_event(event: &str) {
        let now = Local::now();
        let timestamp = now.format("[%Y-%m-%d %H:%M:%S]").to_string();
        let log_message = format!("{} - {}\n", timestamp, event);
        let log_file_path = "log.txt";
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(log_file_path)
            .unwrap();
        writeln!(file, "{}", log_message).unwrap();
    }
}

// driver code
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("need to format command like this: {} <server_id> <port>", args[0]);
        return;
    }

    let server_id = &args[1];
    let port: u16 = args[2].parse().expect("invalid port number");

    let clock_index = match server_id.as_str() {
        "server1" => 0,
        "server2" => 1,
        "server3" => 2,
        _ => {
            eprintln!("{} is an invalid server id", server_id);
            return;
        }
    };

    let server = Arc::new(Mutex::new(Server::new(server_id, clock_index, port)));

    println!("{} listening on port {}", server_id, port);

    // Spawn a thread to handle incoming events
    let server_clone = Arc::clone(&server);
    thread::spawn(move || {
        let mut server = server_clone.lock().unwrap();
        println!("Starting to handle events...");
        server.handle_events();
        println!("Stopped handling events.");
    });

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let input = line.unwrap().trim().to_string();
        let mut server = server.lock().unwrap();
        if input == "end" {
            println!("Shutting down {}", server_id);
            break;
        } else if input == "event" {
            server.increment();
            println!("{} clock is now {:?}", server_id, server.clock);
            Server::log_event(&format!("{} had a local event and updated clock to {:?}", server_id, server.clock));
        } else if input == "clock" {
            println!("{} clock: {:?}", server_id, server.clock);
        } else {
            let target_server_id = input;
            let target_port = match target_server_id.as_str() {
                "server1" => 8001,
                "server2" => 8002,
                "server3" => 8003,
                _ => {
                    println!("{} is an invalid server id", target_server_id);
                    continue;
                }
            };
            let to_address = format!("127.0.0.1:{}", target_port);
            server.send_event(&to_address);
        }
    }

    // Wait for the event handling thread to finish
    thread::sleep(std::time::Duration::from_secs(1));
    println!("Main thread exiting.");
}
