// src/bin/robokitty_cli.rs

use robokitty::{initialize_environment, initialize_system};
use robokitty::commands::cli::{parse_cli_args, execute_command};
use robokitty::lock;
use std::{env, io};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    initialize_environment();
    
    let args: Vec<String> = env::args().collect();
    let command = parse_cli_args(&args)?;

    let (mut budget_system, config) = initialize_system().await?;
    
    lock::create_lock_file()?;
    
    let mut stdout = io::stdout();
    let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
    
    budget_system.save_state()?;
    lock::remove_lock_file()?;
    
    result
}