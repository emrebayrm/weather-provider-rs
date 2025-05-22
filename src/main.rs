use reqwest;
use serde::Deserialize;
use std::{env, time::Duration};
use rumqttc::{MqttOptions, Client, QoS};
use log::{info, error};
use tokio::sync::watch;
use rumqttc::{Packet, Event};

#[derive(Debug, Deserialize)]
struct WeatherResponse {
    current_weather: Option<CurrentWeather>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct CurrentWeather {
    temperature: f64,
    windspeed: f64,
    winddirection: f64,
    time: String,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct DailyForecast {
    time: Vec<String>,
    temperature_2m_max: Vec<f64>,
    temperature_2m_min: Vec<f64>,
    weathercode: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct ForecastResponse {
    daily: DailyForecast,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct Coordinate{
    latitude: f64,
    longtitude: f64,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct Configuration {
    interval_seconds: u64,
    coordinate: Coordinate,
}

fn describe_weather_code(code: u8) -> &'static str {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51..=57 => "Drizzle",
        61..=67 => "Rain",
        71..=77 => "Snow",
        80..=82 => "Showers",
        95 => "Thunderstorm",
        _ => "Unknown",
    }
}

#[tokio::main]
async fn main() {
    // Read from ENV (e.g. set MQTT_USERNAME and MQTT_PASSWORD before running)
    let username = env::var("MQTT_USERNAME").ok();
    let password = env::var("MQTT_PASSWORD").ok();
    let host = env::var("MQTT_HOST").expect("Host should be given");

    env_logger::init();
    info!("Starting Weather MQTT Publisher...");

    // Configure MQTT client
    let mut mqttoptions = MqttOptions::new("weather-client", host, 1883);
    mqttoptions
        .set_keep_alive(Duration::from_secs(60));

    // Only set credentials when both are provided
    if let (Some(u), Some(p)) = (username, password) {
        mqttoptions.set_credentials(&u, &p);
        info!("Using MQTT credentials from env");
    } else {
        info!("No MQTT credentials provided, connecting anonymously");
    }

    let (mut mqtt_client, mut connection) = Client::new(mqttoptions, 10);

    let default_config = Configuration{interval_seconds: 10, coordinate: Coordinate{latitude: 52.0155872, longtitude: 4.3497796}}; 

    let (config_tx, config_rx) = watch::channel(default_config);

    // Subscribe to config topic
    mqtt_client.subscribe("weather/configs", QoS::AtLeastOnce).unwrap();

    let config_tx = config_tx.clone(); // move into task
    tokio::spawn(async move {
        loop {
            match connection.eventloop.poll().await {
                Ok(Event::Incoming(Packet::Publish(publish))) => {
                    if publish.topic == "weather/configs" {
                        if let Ok(cfg) = serde_json::from_slice::<Configuration>(&publish.payload) {
                            log::info!("Received new config: {:?}", cfg);
                            let _ = config_tx.send(cfg);
                        } else {
                            log::warn!("Invalid config format received");
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    log::error!("MQTT error: {}", e);
                    break;
                }
            }
        }
    });

    loop {
        let mut current_config = config_rx.borrow().clone();
        let mut elapsed = 0;
        
        while elapsed < current_config.interval_seconds {
            tokio::time::sleep(Duration::from_secs(1)).await;
        
            let latest = config_rx.borrow();
            if latest.interval_seconds != current_config.interval_seconds {
                // Config changed; restart with new interval
                current_config = latest.clone();
                elapsed = 0;
            } else {
                elapsed += 1;
            }
        }

        match fetch_weather_current(&current_config.coordinate).await {
            Ok(weather) => {
                let json = serde_json::to_string(&weather).unwrap();
                let topic = "weather/current";

                println!("{}", json.to_string());

                match mqtt_client.publish(topic, QoS::AtLeastOnce, true, json) {
                    Ok(_) => info!("Published weather to MQTT"),
                    Err(e) => error!("Failed to publish to MQTT: {}", e),
                }
            }
            Err(e) => error!("Failed to fetch weather: {}", e),
        }

        match fetch_weather_next_5days(&current_config.coordinate).await {
            Ok(weather) => {
                let json = serde_json::to_string(&weather).unwrap();
                let topic = "weather/estimation";

                println!("{}", json.to_string());

                match mqtt_client.publish(topic, QoS::AtLeastOnce, true, json) {
                    Ok(_) => info!("Published weather to MQTT"),
                    Err(e) => error!("Failed to publish to MQTT: {}", e),
                }
            }
            Err(e) => error!("Failed to fetch weather: {}", e),
        }
    }
}

async fn fetch_weather_next_5days(coordinate: &Coordinate) -> Result<DailyForecast, Box<dyn std::error::Error>> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&daily=weathercode,temperature_2m_max,temperature_2m_min&forecast_days=5&timezone=auto",
        coordinate.latitude,
        coordinate.longtitude
    );
    let resp = reqwest::get(url).await?.json::<ForecastResponse>().await?;

    Ok(resp.daily)
}

async fn fetch_weather_current(coordinate: &Coordinate) -> Result<CurrentWeather, Box<dyn std::error::Error>> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current_weather=true",
        coordinate.latitude,
        coordinate.longtitude
    );
    let resp = reqwest::get(url).await?.json::<WeatherResponse>().await?;

    Ok(resp.current_weather.expect("Failed"))
}

