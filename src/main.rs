mod p00_smoke_test;
mod p01_prime_time;
mod p02_means_to_an_end;
mod p03_budget_chat;
mod p04_unusual_database_program;
mod p05_mob_in_the_middle;
mod p06_speed_daemon;

type BoxedErr = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() -> Result<(), BoxedErr> {
    let arg = std::env::args()
        .nth(1)
        .ok_or("Requires one argument - the problem number.")?;
    let problem: u32 = arg.parse()?;

    match problem {
        0 => p00_smoke_test::run(None).await,
        1 => p01_prime_time::run().await,
        2 => p02_means_to_an_end::run().await,
        3 => p03_budget_chat::run().await,
        4 => p04_unusual_database_program::run().await,
        5 => p05_mob_in_the_middle::run().await,
        6 => p06_speed_daemon::run(None).await,
        _ => todo!(),
    }
    .await??;

    Ok(())
}
