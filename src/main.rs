mod types;
mod handler;

use std::env;
use std::error::Error;

use crate::handler::Handler;
use crate::types::Record;
use csv::ReaderBuilder;

fn main() -> Result<(), Box<dyn Error>> {
    let path = env::args().nth(1).expect("missing file argument");

    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(path)?;

    let mut handler = Handler::new();

    for result in rdr.deserialize() {
        let record: Record = result?;

        match record.r#type.as_str() {
            "deposit" => {
                handler.deposit(record.client, record.tx, record.amount);
            }

            "withdrawal" => {
                handler.withdraw(record.client, record.tx, record.amount);
            }

            "dispute" => {
                handler.dispute(record.client, record.tx);
            }

            "resolve" => {
                handler.resolve(record.client, record.tx);
            }

            "chargeback" => {
                handler.chargeback(record.client, record.tx);
            }

            _ => {}
        }
    }

    println!("client,available,held,total,locked");

    let clients = handler.get_clients();
    for (id, client) in clients {
        let total = client.available + client.held;

        println!(
            "{},{},{:.},{:.},{}",
            id,
            client.available,
            client.held,
            total,
            client.locked
        );
    }

    Ok(())
}
