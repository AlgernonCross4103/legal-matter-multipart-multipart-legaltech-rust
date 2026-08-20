use legal_matter_multipart::infrai::InfraiClient;
use legal_matter_multipart::matter_intake::{deliver_signed_document, MatterIntake};
use std::env;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("matter-intake: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        return Err("usage: matter-intake <matter-id> <signed-document> <recipient> <days-until-deadline>".into());
    }
    let intake = MatterIntake {
        matter_id: args[1].clone(),
        signed_document: PathBuf::from(&args[2]),
        recipient: args[3].clone(),
        days_until_deadline: args[4].parse()?,
    };
    let client = InfraiClient::from_env()?;
    let receipt = deliver_signed_document(&client, "legal-matter-intake", intake).await?;
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}

