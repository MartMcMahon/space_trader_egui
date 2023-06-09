#![deny(clippy::all)]
#![forbid(unsafe_code)]

use directories::UserDirs;
use eframe::egui;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use serde_derive::{Deserialize, Serialize};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;
use tokio::runtime::Runtime;

use std::fs::File;
use std::io::prelude::*;

struct STApp {
    // Sender/Receiver for async notifications.
    tx: Sender<DataPack>,
    rx: Receiver<DataPack>,

    // Silly app state.
    state: AppState,
    token: String,

    value: u32,
    count: u32,
    credits: u32,
    corpo_name: String,
    validation_text: String,
    main_text: String,
    error_text: String,

    agent_data: AgentData,
    waypoint: Waypoint,
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

    let file = File::open(token_file_path);
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

fn write_token(token: String) -> Result<usize, std::io::Error> {
    let user_dirs = UserDirs::new().unwrap();
    let token_file_path = user_dirs.home_dir().join(".space_traders/token");
    let mut file = File::create(token_file_path).expect("creation failed");
    file.write(token.as_bytes())
}

fn main() {
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

    // initilize app object
    let token_res = read_token();
    let main_app = match token_res {
        Ok(token) => STApp::with_token(token),
        Err(_) => STApp::default(),
    };

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
            state: AppState::Main,
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
            main_text: String::new(),
            error_text: String::new(),

            agent_data: AgentData::default(),
            waypoint: Waypoint::default(),
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
                    self.validation_text =
                        serde_json::from_value(rec.data.get("token").unwrap().clone()).unwrap();

                    write_token(self.validation_text.clone()).unwrap();
                }
            }
            AppState::Main => {
                // Update the counter with the async response.
                if let Ok(rec) = self.rx.try_recv() {
                    // println!("{:#?}", rec);
                    match rec {
                        DataPack {
                            data_type: DataType::Agent,
                            data: res,
                        } => {
                            self.agent_data = serde_json::from_value(res).unwrap();
                        }
                        DataPack {
                            data_type: DataType::Waypoint,
                            data: res,
                        } => {
                            self.waypoint = serde_json::from_value(res).unwrap();
                        }
                    }
                    // serde_json::from_value(rec.get("data").unwrap().clone()).unwrap();
                }

                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.label("You are logged in");
                    ui.add(egui::Slider::new(&mut self.value, 1..=120).text("value"));
                    ui.label(self.main_text.clone());
                    ui.label(self.agent_data.account_id.clone());
                    ui.label("credits");
                    ui.label(self.agent_data.credits.to_string());
                    ui.label("current waypoint");
                    ui.label(self.agent_data.headquarters.clone());
                    ui.label(self.agent_data.headquarters.clone());

                    if ui.button("get data").clicked() {
                        agent_data_request(self.tx.clone(), ctx.clone(), self.token.clone());
                        waypoint_request(
                            self.agent_data.headquarters.clone(),
                            self.tx.clone(),
                            ctx.clone(),
                            self.token.clone(),
                        )
                    }
                });
            }
        }
    }
}

fn register_request(tx: Sender<DataPack>, ctx: egui::Context) {
    let register_url = "https://api.spacetraders.io/v2/register".to_owned();
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
        let _ = tx.send(DataPack {
            data_type: DataType::Agent,
            data: res,
        });
        ctx.request_repaint();
    });
}

fn agent_data_request(tx: Sender<DataPack>, ctx: egui::Context, token: String) {
    let agent_data_url = "https://api.spacetraders.io/v2/my/agent";
    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        HeaderValue::from_str("application/json").unwrap(),
    );
    headers.insert(
        "Authorization",
        HeaderValue::from_str(("Bearer ".to_owned() + token.as_str()).as_str()).unwrap(),
    );

    tokio::spawn(async move {
        let res: serde_json::Value = Client::default()
            .get(agent_data_url)
            .headers(headers)
            .send()
            .await
            .expect("agent data request failed")
            .json()
            .await
            .unwrap();

        let _ = tx.send(DataPack {
            data_type: DataType::Agent,
            data: res.get("data").unwrap().clone(),
        });

        ctx.request_repaint();
    });
}

#[derive(Deserialize, Serialize)]
struct AgentRequest {}

#[derive(Default, Deserialize, Serialize)]
struct AgentData {
    #[serde(rename = "accountId")]
    account_id: String,
    credits: i64,
    headquarters: String,
    #[serde(rename = "startingFaction")]
    starting_faction: String,
    symbol: String,
}

fn waypoint_request(waypoint: String, tx: Sender<DataPack>, ctx: egui::Context, token: String) {
    if waypoint.len() == 0 {
        return;
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        HeaderValue::from_str("application/json").unwrap(),
    );
    headers.insert(
        "Authorization",
        HeaderValue::from_str(("Bearer ".to_owned() + token.as_str()).as_str()).unwrap(),
    );

    tokio::spawn(async move {
        let res: serde_json::Value = Client::default()
            .get(waypoint_url_builder(&waypoint))
            .headers(headers)
            .send()
            .await
            .expect("agent data request failed")
            .json()
            .await
            .unwrap();

        println!("{:#?}", res.get("data").unwrap());

        let _ = tx.send(DataPack {
            data_type: DataType::Waypoint,
            data: res.get("data").unwrap().clone(),
        });
        ctx.request_repaint();
    });
}

fn waypoint_url_builder(waypoint: &str) -> String {
    let parts: Vec<&str> = waypoint.split("-").collect();
    println!("waypoint, {}", waypoint);
    format!(
        "https://api.spacetraders.io/v2/systems/{}/waypoints/{}",
        parts[..2].join("-"),
        parts.join("-")
    )
}

enum DataType {
    Agent,
    Waypoint,
}

struct DataPack {
    data_type: DataType,
    data: serde_json::Value,
}

#[derive(Default, Deserialize, Serialize)]
struct Waypoint {
    #[serde(rename = "systemSymbol")]
    system_symbol: String,
    symbol: String,
    #[serde(rename = "type")]
    waypoint_type: String,
    x: i32,
    y: i32,
    orbitals: Vec<serde_json::Value>,
    traits: Vec<serde_json::Value>,
    // { "symbol": "OVERCROWDED",
    //   "name": "Overcrowded",
    //   "description": "A waypoint teeming with inhabitants, leading to cramped living conditions and a high demand for resources." },
    chart: serde_json::Value,
    // { "submittedBy": "COSMIC", "submittedOn": "2023-06-10T15:55:44.111Z" },
    faction: Faction,
}
#[derive(Default, Deserialize, Serialize)]
enum FactionName {
    #[default]
    COSMIC,
}
#[derive(Default, Deserialize, Serialize)]
struct Faction {
    symbol: FactionName,
}
