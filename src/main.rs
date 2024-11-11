use std::{env, str::FromStr};

use bitcoin::{
    bip32::{ChildNumber, DerivationPath, Xpub},
    key::Secp256k1,
    Address, KnownHrp, Network
};

use clap::Parser;
use dotenv::dotenv;

mod api;

#[derive(Debug, Clone, PartialEq)]
enum ScriptPubKeyType {
    /// Legacy type (1...)
    P2PKH,
    /// Wrapped segwit type (3...)
    P2SHWPKH,
    /// Segwit type (bc1q...)
    P2WPKH,
    /// Taproot type (bc1p...)
    P2TR,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The xpub to scan (--xpub or --query is required)
    #[clap(index = 1, required = false)]
    xpub: Option<String>,

    /// The deriviation path to scan [default: 0/0]
    #[clap(short, long)]
    path: Option<String>,

    /// Number of addresses to scan [default: 10]
    #[clap(short, long)]
    count: Option<u32>,

    /// The ScriptPubKey type (P2PKH,P2SHWPKH,P2WPKH,P2TR) [default: all]
    #[clap(short, long, value_delimiter = ',', required = false)]
    r#type: Option<Vec<String>>,

    /// An address to query (calls the api, displays the response, and exits). This switch is checked first
    #[clap(short, long, required = false)]
    query: Option<String>,
}

fn get_value<T>(opt: Option<T>, key: &str, defval: T) -> T
where
    T: FromStr,
{
    if let Some(value) = opt {
        return value;
    }

    if let Ok(env_value) = env::var(key) {
        if let Ok(parsed_value) = env_value.parse::<T>() {
            return parsed_value;
        }
    }

    defval
}

fn get_vec<T>(opt: Option<Vec<T>>, key: &str, defval: Vec<T>) -> Vec<T>
where
    T: FromStr,
{
    if let Some(value) = opt {
        return value;
    }

    if let Ok(env_value) = env::var(&key) {
        let parsed_value: Result<Vec<T>, _> = env_value
            .split(',')
            .map(|s| s.trim().parse::<T>())
            .collect();

        if let Ok(parsed_value) = parsed_value {
            return parsed_value;
        }
    }

    defval
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenv().ok();

    let args = Args::parse();

    if let Some(query) = args.query {
        api::display_api_response(query).await?;
        return Ok(());
    }

    let val = match args.xpub {
        Some(v) => v,
        None => env::var("SCAN_XPUB").expect("SCAN_XPUB must be set when --query is omitted, or passed in as the first positional arg (xpub-scan xpub123abc...)")
    };

    let secp = Secp256k1::new();
    let xpub = Xpub::from_str(&val)?;
    let path = get_value(args.path, "SCAN_PATH", "0/0".to_string());
    let deriv_path = DerivationPath::from_str(&path)?;
    let mut cn = deriv_path.into_iter().last().unwrap().clone();
    let path_except_last = get_path_except_last(&deriv_path);
    let types_vec = get_vec(args.r#type, "SCAN_SCRIPTPUBKEY_TYPE", vec![]);
    let addr_types = parse_enum_values(types_vec)?;
    let count = get_value(args.count, "SCAN_COUNT", 10);
    let start: u32 = cn.into();
    let limit = start + count;

    let mut pubkeys = Vec::new();

    while limit > cn.into() {
        let dp = path_except_last.child(cn);
        pubkeys.push(xpub.derive_pub(&secp, &dp)?);
        cn = cn.increment()?;
    }

    if addr_types.is_empty() || addr_types.contains(&ScriptPubKeyType::P2PKH) {
        for pk in &pubkeys {
            let addr = Address::p2pkh(&pk.to_pub(), Network::Bitcoin);
            let bal = api::get_address_sats(addr.to_string()).await?;
            println!("{}: {}", addr, bal);
        }
    }

    if addr_types.is_empty() || addr_types.contains(&ScriptPubKeyType::P2SHWPKH) {
        for pk in &pubkeys {
            let addr = Address::p2shwpkh(&pk.to_pub(), Network::Bitcoin);
            let bal = api::get_address_sats(addr.to_string()).await?;
            println!("{}: {}", addr, bal);
        }
    }

    if addr_types.is_empty() || addr_types.contains(&ScriptPubKeyType::P2WPKH) {
        for pk in &pubkeys {
            let addr = Address::p2wpkh(&pk.to_pub(), KnownHrp::Mainnet);
            let bal = api::get_address_sats(addr.to_string()).await?;
            println!("{}: {}", addr, bal);
        }
    }

    if addr_types.is_empty() || addr_types.contains(&ScriptPubKeyType::P2TR) {
        for pk in &pubkeys {
            let addr = Address::p2tr(&secp, pk.to_x_only_pub(), None, KnownHrp::Mainnet);
            let bal = api::get_address_sats(addr.to_string()).await?;
            println!("{}: {}", addr, bal);
        }
    }

    Ok(())
}

fn parse_enum_values(values: Vec<String>) -> Result<Vec<ScriptPubKeyType>, anyhow::Error> {
    let mut enums = Vec::new();
    
    for value in values {
        match value.to_uppercase().as_str() {
            "P2PKH" => enums.push(ScriptPubKeyType::P2PKH),
            "P2SHWPKH" => enums.push(ScriptPubKeyType::P2SHWPKH),
            "P2WPKH" => enums.push(ScriptPubKeyType::P2WPKH),
            "P2TR" => enums.push(ScriptPubKeyType::P2TR),
            _ => return Err(anyhow::anyhow!("Invalid enum value: {}", value)),
        }
    }
    
    Ok(enums)
}

/// Gets the derivation path excluding the last component (i.e. index)
fn get_path_except_last(path: &DerivationPath) -> DerivationPath {
    let child_numbers: Vec<&ChildNumber>  = path.into_iter().collect();
    let all_except_last = &child_numbers[..child_numbers.len() - 1];
    let cloned_all_except_last: Vec<ChildNumber> = all_except_last.iter().map(|&x| x.clone()).collect();
    let path_except_last = DerivationPath::from_iter(cloned_all_except_last);
    path_except_last
}