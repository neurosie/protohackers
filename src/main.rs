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

    match problem {
        0 => p00_smoke_test::run().await,
        1 => p01_prime_time::run().await,
        2 => p02_means_to_an_end::run().await,
        3 => p03_budget_chat::run().await,
        _ => todo!(),
    }?;

    Ok(())
}
