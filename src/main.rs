mod p00_smoke_test;
mod p01_prime_time;
mod p02_means_to_an_end;
mod p03_budget_chat;

use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arg = std::env::args()
        .nth(1)
        .ok_or("Requires one argument - the problem number.")?;
    let problem: u32 = arg.parse()?;

    let listener = TcpListener::bind("0.0.0.0:7878").await?;
    println!("Listening on port 7878");

    match problem {
        0 => p00_smoke_test::run(listener).await,
        1 => p01_prime_time::run(listener).await,
        2 => p02_means_to_an_end::run(listener).await,
        3 => p03_budget_chat::run(listener).await,
        _ => todo!(),
    }?;

    Ok(())
}
