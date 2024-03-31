use serde::{Deserialize, Serialize};
use serde_json::to_string;
use std::collections::HashMap;
use std::env;
use std::io::{self, BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

// vector clock represented by a hashmap for better sparse representation
type VectorClock = HashMap<String, i32>;

// server message structure
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ServerMessage {
    sender_id: String,
    clock: VectorClock,
}

// tcp server structure
#[derive(Debug)]
struct Server {
    id: String,
    clock: VectorClock,
    listener: TcpListener,
}

// clone override for tcp server
impl Clone for Server {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            clock: self.clock.clone(),
            listener: TcpListener::bind(format!("127.0.0.1:{}", self.listener.local_addr().unwrap().port())).unwrap(),
        }
    }
}

// server implementation
impl Server {

    // binds server to port
    fn new(id: &str, port: u16) -> Self {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
        let mut clock = VectorClock::new();
        clock.insert(id.to_string(), 0);
        Self {
            id: id.to_string(),
            clock,
            listener,
        }
    }

    // increments logical time
    fn increment(&mut self) {
        *self.clock.entry(self.id.clone()).or_insert(0) += 1;
    }

    // updates this clock based on larger time value
    fn update_clock(&mut self, other_clock: &VectorClock) {
        for (id, other_time) in other_clock {
            let time = self.clock.entry(id.clone()).or_insert(0);
            *time = (*time).max(*other_time);
        }
        self.increment();
    }

    // sends event to other server at other_address
    fn send_event(&mut self, other_address: &str) {
        self.increment();
        let msg = ServerMessage {
            sender_id: self.id.clone(),
            clock: self.clock.clone(),
        };
        let mut stream = TcpStream::connect(other_address).unwrap();
        let msg_json = to_string(&msg).unwrap();
        stream.write_all(msg_json.as_bytes()).unwrap();
    }

    // handles incoming events
    fn handle_events(&mut self) {
        let mut incoming_clocks = Vec::new();
        for stream in self.listener.incoming() {
            let mut stream = stream.unwrap();
            let mut buffer = [0; 1024];
            let _ = stream.read(&mut buffer).unwrap();
            let message: ServerMessage = serde_json::from_slice(&buffer).expect("cannot deserialize message");
            incoming_clocks.push(message.clock);
        }
    
        for clock in incoming_clocks {
            self.update_clock(&clock);
        }
    }
}

// driver code
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("need to format command like this: {} server_id port", args[0]);
        return;
    }

    let server_id = &args[1];
    let port: u16 = args[2].parse().expect("invalid port number");

    let mut server = Server::new(server_id, port);

    println!("server {} listening on port {}", server_id, port);

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let input = line.unwrap();
        if input == "end" {
            break;
        }

        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() == 2 {
            let from_idx: usize = parts[0].parse().unwrap();
            let to_idx: usize = parts[1].parse().unwrap();

            if from_idx == server.id.parse::<usize>().unwrap() && to_idx > 0 && to_idx <= 3 {
                let to_address = format!("127.0.0.1:800{}", to_idx);
                server.send_event(&to_address);
            }
        }
    }

    // handle incoming events in the main thread
    server.handle_events();
}
