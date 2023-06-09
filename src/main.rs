#![deny(clippy::all)]
#![forbid(unsafe_code)]

use directories::UserDirs;
use eframe::egui;
use reqwest::Client;
use serde_derive::{Deserialize, Serialize};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;
use tokio::runtime::Runtime;

use std::fs::File;
use std::io::prelude::*;

struct STApp {
    // Sender/Receiver for async notifications.
    tx: Sender<serde_json::Value>,
    rx: Receiver<serde_json::Value>,

    // Silly app state.
    state: AppState,
    token: String,

    value: u32,
    count: u32,
    credits: u32,
    corpo_name: String,
    validation_text: String,
}

// enum RequestBody {
//     json: serde_json::Value,
// }

#[derive(Deserialize, Serialize)]
struct RegisterResultBody {
    data: serde_json::Value,
}

#[derive(Deserialize, Serialize)]
struct HttpbinJson {
    json: Body,
}

#[derive(Deserialize, Serialize)]
struct RegisterRequestBody {
    json: RegisterRequest,
}

#[derive(Deserialize, Serialize)]
struct Body {
    incr: u32,
}

#[derive(Deserialize, Serialize)]
struct RegisterRequest {
    symbol: String,
    faction: String,
}

#[derive(Deserialize, Serialize)]
struct RegisterResult {
    data: serde_json::Value,
}

struct TokenReadError;
enum AppState {
    Login,
    Main,
}

fn read_token() -> Result<String, TokenReadError> {
    let user_dirs = UserDirs::new().unwrap();
    let token_file_path = user_dirs.home_dir().join(".space_traders/token");

    let mut file = File::open(token_file_path);
    match file {
        Ok(mut f) => {
            let mut token = String::new();
            f.read_to_string(&mut token).unwrap();
            // println!("{}", &token);
            Ok(token)
        }
        Err(_) => {
            println!("error reading file");
            Err(TokenReadError)
        }
    }
}

fn main() {
    // initilize app object
    let token = read_token();
    let main_app = match token {
        Ok(token) => {
            println!("{token}");
            STApp::with_token(token)
        }
        Err(_) => STApp::default(),
    };

    // create runtime
    let rt = Runtime::new().expect("Unable to create Runtime");
    // Enter the runtime so that `tokio::spawn` is available immediately.
    let _enter = rt.enter();
    // Execute the runtime in its own thread.
    // The future doesn't have to do anything. In this example, it just sleeps forever.
    std::thread::spawn(move || {
        rt.block_on(async {
            loop {
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        })
    });

    // Run the GUI in the main thread.
    eframe::run_native(
        "Space Traders",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Box::new(main_app)),
    );
}

impl STApp {
    fn with_token(token: String) -> Self {
        Self {
            token,
            ..Default::default()
        }
    }
}

impl Default for STApp {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();

        Self {
            tx,
            rx,
            value: 1,
            count: 0,
            credits: 0,

            state: AppState::Login,
            token: String::new(),

            corpo_name: String::new(),
            validation_text: String::new(),
        }
    }
}

impl eframe::App for STApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        match self.state {
            AppState::Login => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    // split into two panels
                    ui.label(format!("token: {}", self.token));

                    ui.label("corpo name:");
                    ui.add(egui::TextEdit::singleline(&mut self.corpo_name));
                    ui.label(self.validation_text.clone());

                    if ui.button(format!("register")).clicked() {
                        register_request(self.tx.clone(), ctx.clone());
                        // self.validation_text = "ok!".to_owned();
                    }
                });

                if let Ok(rec) = self.rx.try_recv() {
                    // self.count += incr;
                    // println!("recieved {:#?}", rec);
                    self.validation_text = serde_json::from_value(
                        rec.get("data").unwrap().get("token").unwrap().clone(),
                    )
                    .unwrap();
                }
            }
            AppState::Main => {
                // Update the counter with the async response.
                if let Ok(rec) = self.rx.try_recv() {
                    // self.count += incr;
                    // println!("recieved {:#?}", rec);
                    self.validation_text = serde_json::from_value(
                        rec.get("data").unwrap().get("token").unwrap().clone(),
                    )
                    .unwrap();
                }

                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.label("Press the button to initiate an HTTP request.");
                    ui.label("If successful, the count will increase by the following value.");

                    ui.add(egui::Slider::new(&mut self.value, 1..=120).text("value"));

                    if ui.button(format!("Count: {}", self.count)).clicked() {
                        register_request(self.tx.clone(), ctx.clone());
                    }
                });
            }
        }
    }
}

// fn send_req(incr: u32, tx: Sender<u32>, ctx: egui::Context) {
//     tokio::spawn(async move {
//         // Send a request with an increment value.
//         let body: HttpbinJson = Client::default()
//             .post("https://httpbin.org/anything")
//             .json(&Body { incr })
//             .send()
//             .await
//             .expect("Unable to send request")
//             .json()
//             .await
//             .expect("Unable to parse response");

// // After parsing the response, notify the GUI thread of the increment value.
// let _ = tx.send(body.json.incr);
// ctx.request_repaint();
// });
// }

fn register_request(tx: Sender<serde_json::Value>, ctx: egui::Context) {
    // let register_url = "https://api.spacetraders.io/v2/register".to_owned();
    let register_url =
        "https://stoplight.io/mocks/spacetraders/spacetraders/96627693/register".to_owned();
    tokio::spawn(async move {
        let res: serde_json::Value = Client::default()
            .post(register_url)
            .json(&RegisterRequest {
                symbol: "STD_CALLSIGN".to_owned(),
                faction: "COSMIC".to_owned(),
            })
            .send()
            .await
            .expect("register failed")
            .json()
            .await
            .unwrap();

        print!("{:#?}", res);
        let _ = tx.send(res);
        ctx.request_repaint();
    });
}
