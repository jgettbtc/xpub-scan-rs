
use std::{collections::HashMap, env};
use anyhow::Context;
use serde_json::Value;

async fn get_address(addr: String) -> Result<HashMap<String, Value>, anyhow::Error> {
    let api_url_template = env::var("API_ADDRESS_URL_TEMPLATE").expect("API_ADDRESS_URL_TEMPLATE must be set");
    let api_url = api_url_template.replace("{addr}", &addr);

    let response = reqwest::get(api_url)
        .await
        .context("Failed to fetch data")?;

    let addr_response: HashMap<String, Value> = response.json()
        .await
        .context("Failed to parse JSON response")?;

    Ok(addr_response)
}

pub async fn get_address_sats(addr: String) -> Result<u32, anyhow::Error> {
    let path = env::var("API_ADDRESS_BALANCE_PATH").expect("API_ADDRESS_BALANCE_PATH must bet set");
    let unit = env::var("API_ADDRESS_BALANCE_UNIT").expect("API_ADDRESS_BALANCE_UNIT must bet set");
    let map = get_address(addr).await?;
    let bal: u32 = get_value_by_path(&map, &path)?;
    match unit.as_str() {
        "btc" => Ok(bal / 100_000_000),
        "sat" => Ok(bal),
        _ => Err(anyhow::anyhow!("Unit must be btc or sat")),
    }
}
    
pub async fn display_api_response(addr: String) -> Result<(), anyhow::Error> {
    let map = get_address(addr).await?;
    let json = serde_json::to_string_pretty(&map)?;
    Ok(println!("{}", json))
}

fn get_value_by_path<T>(map: &HashMap<String, Value>, path: &str) -> Result<T, anyhow::Error>
where
    T: serde::de::DeserializeOwned,
{
    let keys: Vec<&str> = path.split('.').collect();
    let mut current_value = Value::Object(map.clone().into_iter().collect());

    for key in keys {
        match current_value {
            Value::Object(ref mut obj) => {
                if let Some(value) = obj.remove(key) {
                    current_value = value;
                } else {
                    return Err(anyhow::anyhow!("Key '{}' not found in JSON path", key));
                }
            }
            _ => return Err(anyhow::anyhow!("Expected an object at key '{}'", key)),
        }
    }

    match T::deserialize(current_value) {
        Ok(value) => Ok(value),
        Err(_) => Err(anyhow::anyhow!("Failed to deserialize value at path '{}'", path)),
    }
}