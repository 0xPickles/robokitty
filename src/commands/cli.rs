// src/commands/cli.rs
use chrono::{DateTime, NaiveDate, Utc};
use std::{collections::HashMap, io::Write};
use std::{fs, error::Error};

use crate::core::models::VoteChoice;
use crate::core::budget_system::BudgetSystem;
use crate::app_config::AppConfig;
use super::common::{BudgetRequestDetailsCommand, Command, CommandExecutor, UpdateTeamDetails, UpdateProposalDetails};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "robokitty")]
#[command(about = "Budget system management CLI", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage teams
    Team {
        #[command(subcommand)]
        command: TeamCommands,
    },
    
    /// Manage epochs  
    Epoch {
        #[command(subcommand)]
        command: EpochCommands, 
    },
    /// Manage proposals
    Proposal {
        #[command(subcommand)]
        command: ProposalCommands,
    },
    /// Manage voting
   Vote {
    #[command(subcommand)]
    command: VoteCommands,
    },
    /// Manage raffles 
    Raffle {
        #[command(subcommand)]
        command: RaffleCommands,
    },
    /// Print reports
    Report {
        #[command(subcommand)]
        command: ReportCommands,
    },
    /// Import existing raffles and votes
    Import {
        #[command(subcommand)]
        command: ImportCommands,
    },
    /// Run JSON script
    RunScript {
        script_file_path: Option<String>,
    }, 
}

#[derive(Subcommand)]
pub enum TeamCommands {
    /// Add a new team to the system
    Add {
        /// Team's display name
        #[arg(long, value_name = "NAME")]
        name: String,
        
        /// Team's representative contact
        #[arg(long, value_name = "REPRESENTATIVE")] 
        representative: String,
        
        /// Monthly revenue values (comma separated)
        #[arg(long, value_name = "REVENUE")]
        revenue: Option<String>,
        
        /// Ethereum payment address
        #[arg(long, value_name = "ADDRESS")]
        address: Option<String>,
    },

    /// Update an existing team
    Update {
        /// Current team name
        #[arg(value_name = "TEAM")]
        name: String,
        
        /// New team name
        #[arg(long, value_name = "NAME")]
        new_name: Option<String>,
        
        /// New representative
        #[arg(long, value_name = "REPRESENTATIVE")]
        representative: Option<String>,
        
        /// New status (Earner/Supporter/Inactive)
        #[arg(long, value_name = "STATUS")]
        status: Option<String>,
        
        /// New revenue values
        #[arg(long, value_name = "REVENUE")]
        revenue: Option<String>,
        
        /// New payment address 
        #[arg(long, value_name = "ADDRESS")]
        address: Option<String>,
    }
}

#[derive(Subcommand)] 
pub enum EpochCommands {
    /// Create a new epoch period
    Create {
        /// Epoch name/identifier
        #[arg(value_name = "NAME")]
        name: String,
        
        /// Start date (YYYY-MM-DD)
        #[arg(value_name = "START_DATE")]
        start_date: String,
        
        /// End date (YYYY-MM-DD)
        #[arg(value_name = "END_DATE")]
        end_date: String,
    },

    /// Activate an epoch for proposals
    Activate {
        /// Epoch name to activate
        #[arg(value_name = "NAME")]
        name: String,
    },

    /// Set epoch reward amount
    SetReward {
        /// Token symbol (e.g. ETH)
        #[arg(value_name = "TOKEN")]
        token: String,
        
        /// Reward amount
        #[arg(value_name = "AMOUNT")]
        amount: f64,
    },

    /// Close an epoch
    Close {
        /// Optional epoch name (uses active if omitted)
        #[arg(value_name = "NAME")]
        epoch_name: Option<String>,
    }
}

#[derive(Subcommand)]
pub enum ProposalCommands {
   /// Add a new proposal
   Add {
       /// Proposal title
       #[arg(long, value_name = "TITLE")]
       title: String,

       /// Proposal URL
       #[arg(long, value_name = "URL")] 
       url: Option<String>,
       
       /// Team name
       #[arg(long, value_name = "TEAM")]
       team: Option<String>,
       
       /// Request amounts (format: ETH:100.5,USD:1000)
       #[arg(long, value_name = "AMOUNTS")]
       amounts: Option<String>,
       
       /// Start date (YYYY-MM-DD)
       #[arg(long, value_name = "START")]
       start: Option<String>,
       
       /// End date (YYYY-MM-DD)
       #[arg(long, value_name = "END")]
       end: Option<String>,
       
       /// Is loan request
       #[arg(long)]
       loan: Option<bool>,
       
       /// Payment address
       #[arg(long, value_name = "ADDRESS")]
       address: Option<String>,
    
       /// Date announced (YYYY-MM-DD)
       #[arg(long, value_name = "ANNOUNCED")]
       announced_at: Option<String>,

       /// Date published (YYYY-MM-DD)
       #[arg(long, value_name = "PUBLISHED")] 
       published_at: Option<String>,
   },

   /// Update an existing proposal 
   Update {
       /// Proposal name to update
       #[arg(value_name = "NAME")]
       name: String,
       
       #[arg(long, value_name = "TITLE")]
       title: Option<String>,
       
       #[arg(long, value_name = "URL")]
       url: Option<String>,
       
       #[arg(long, value_name = "TEAM")]
       team: Option<String>,
       
       #[arg(long, value_name = "AMOUNTS")] 
       amounts: Option<String>,
       
       #[arg(long, value_name = "START")]
       start: Option<String>,
       
       #[arg(long, value_name = "END")]
       end: Option<String>,
       
       #[arg(long)]
       loan: Option<bool>,
       
       #[arg(long, value_name = "ADDRESS")]
       address: Option<String>,
           
       /// Date announced (YYYY-MM-DD)
       #[arg(long, value_name = "ANNOUNCED")]
       announced_at: Option<String>,

       /// Date published (YYYY-MM-DD)
       #[arg(long, value_name = "PUBLISHED")] 
       published_at: Option<String>,
   },

   /// Close a proposal
   Close {
       /// Proposal name
       name: String,
       
       /// Resolution (Approved/Rejected/Invalid/Duplicate/Retracted)
       resolution: String,
   },
   
   /// Log payment for approved and unpaid proposals
   Pay {
    /// Proposal names to be marked as paid (comma separated)
    #[arg(value_name = "PROPOSALS")]
    proposals: String,
    
    /// Payment transaction hash
    #[arg(long)]
    tx: String,
    
    /// Payment date (YYYY-MM-DD)  
    #[arg(long)]
    date: String,
}
}

#[derive(Subcommand)]
pub enum VoteCommands {
   /// Process a vote
   Process {
       /// Proposal name
       name: String,
       
       /// Counted votes (format: Team1:Yes,Team2:No)
       #[arg(long, value_name = "COUNTED")]
       counted: String,
       
       /// Uncounted votes (format: Team3:Yes,Team4:No)
       #[arg(long, value_name = "UNCOUNTED")]  
       uncounted: String,
       
       /// Vote opened date (YYYY-MM-DD)
       #[arg(long, value_name = "OPENED")]
       opened: Option<String>,
       
       /// Vote closed date (YYYY-MM-DD)
       #[arg(long, value_name = "CLOSED")]
       closed: Option<String>,
   }
}

#[derive(Subcommand)]
pub enum RaffleCommands {
   /// Create a new raffle
   Create {
       /// Proposal name
       name: String,
       
       /// Block offset
       #[arg(long, value_name = "OFFSET")]
       block_offset: Option<u64>,
       
       /// Excluded teams (comma separated)
       #[arg(long, value_name = "EXCLUDED")]
       excluded: Option<String>,
   }
}

#[derive(Subcommand)]
pub enum ReportCommands {
   /// Print team report, optionally for a specific team
   Team {
       #[arg(value_name = "TEAM")]
       team_name: Option<String>,
   },

   /// Print epoch state report
   EpochState,

   /// Print team vote participation
   TeamParticipation {
       team_name: String,
       epoch_name: Option<String>,
   },

   /// Print point report
   Points {
       #[arg(long, value_name = "EPOCH")]
       epoch_name: Option<String>,
   },

   /// Generate closed proposals report
   ClosedProposals {
       #[arg(value_name = "EPOCH")]
       epoch_name: String,
   },

   /// Generate end of epoch report
   EndOfEpoch {
       #[arg(value_name = "EPOCH")] 
       epoch_name: String,
   },

   /// Generate unpaid requests report
   UnpaidRequests {
       #[arg(long, value_name = "PATH")]
       output_path: Option<String>,
       #[arg(long, value_name = "EPOCH")]
       epoch_name: Option<String>,
   },

   /// Generate epoch payments report
   EpochPayments {
        /// Epoch name
        #[arg(value_name = "EPOCH")]
        epoch_name: String,
        
        /// Output file path
        #[arg(long, value_name = "PATH")]
        output: Option<String>,
    },

   /// Generate report for specific proposal
   ForProposal {
       #[arg(value_name = "PROPOSAL")]
       proposal_name: String,
   },

   /// Generate a summary report across all epochs
   AllEpochs {
        /// Optional path to save the report markdown file
        #[arg(long, value_name = "PATH")]
        output_path: Option<String>,

        /// Only include data from closed epochs in the report
        #[arg(long)]
        only_closed: bool,
    },
}


#[derive(Subcommand)]
pub enum ImportCommands {
   /// Import a predefined raffle
   PredefinedRaffle {
       proposal_name: String,
       counted_teams: Vec<String>,
       uncounted_teams: Vec<String>, 
       total_counted_seats: usize,
       max_earner_seats: usize,
   },

   /// Import historical vote
   HistoricalVote {
       proposal_name: String,
       passed: bool,
       participating_teams: Vec<String>,
       non_participating_teams: Vec<String>,
       counted_points: Option<u32>,
       uncounted_points: Option<u32>,
   },

   /// Import historical raffle
   HistoricalRaffle {
       proposal_name: String,
       initiation_block: u64,
       randomness_block: u64,
       team_order: Option<Vec<String>>,
       excluded_teams: Option<Vec<String>>,
       total_counted_seats: Option<usize>,
       max_earner_seats: Option<usize>,
   }
}


fn parse_eth_address(addr: &str) -> Result<String, String> {
    if !addr.starts_with("0x") {
        return Err("Ethereum address must start with 0x".into());
    }
    if addr.len() != 42 {
        return Err("Ethereum address must be 42 characters long".into());
    }
    // Basic hex check
    if !addr[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid hex characters in address".into());
    }
    Ok(addr.to_string())
}

fn parse_votes(votes_str: &str) -> Result<HashMap<String, VoteChoice>, Box<dyn Error>> {
    votes_str
        .split(',')
        .map(|vote| {
            let parts: Vec<&str> = vote.split(':').collect();
            if parts.len() != 2 {
                return Err("Invalid vote format. Expected Team:Choice".into());
            }
            let choice = match parts[1].to_lowercase().as_str() {
                "yes" => VoteChoice::Yes,
                "no" => VoteChoice::No,
                _ => return Err(format!("Invalid vote choice: {}. Must be Yes or No", parts[1]).into()),
            };
            Ok((parts[0].to_string(), choice))
        })
        .collect()
}


impl Cli {
    pub fn into_command(self) -> Result<Command, Box<dyn Error>> {
        match self.command {

            Commands::Team { command } => match command {
                TeamCommands::Add { name, representative, revenue, address } => {
                    if let Some(addr) = &address {
                        parse_eth_address(addr)?;
                    }
                    
                    let parsed_revenue = revenue.map(|rev| {
                        rev.split(',')
                           .map(|v| v.parse::<u64>())
                           .collect::<Result<Vec<_>, _>>()
                    }).transpose()?;

                    Ok(Command::AddTeam {
                        name,
                        representative,
                        trailing_monthly_revenue: parsed_revenue,
                        address
                    })
                },
                TeamCommands::Update { name, new_name, representative, status, revenue, address } => {
                    Ok(Command::UpdateTeam {
                        team_name: name,
                        updates: UpdateTeamDetails {
                            name: new_name,
                            representative,
                            status,
                            trailing_monthly_revenue: revenue.map(|rev| {
                                rev.split(',')
                                   .map(|v| v.parse::<u64>().unwrap())
                                   .collect()
                            }),
                            address
                        }
                    })
                }
            },

            Commands::Epoch { command } => match command {
                EpochCommands::Create { name, start_date, end_date } => {
                    let start = DateTime::parse_from_rfc3339(&start_date)?
                        .with_timezone(&Utc);
                    let end = DateTime::parse_from_rfc3339(&end_date)?
                        .with_timezone(&Utc);
                    Ok(Command::CreateEpoch { name, start_date: start, end_date: end })
                },
                EpochCommands::Activate { name } => {
                    Ok(Command::ActivateEpoch { name })
                },
                EpochCommands::SetReward { token, amount } => {
                    Ok(Command::SetEpochReward { token, amount }) 
                },
                EpochCommands::Close { epoch_name } => {
                    Ok(Command::CloseEpoch { epoch_name })
                }
            },

            Commands::Proposal { command } => match command {
                ProposalCommands::Add { title, url, team, amounts, start, end, loan, address, announced_at, published_at } => {
                    let published = published_at.map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d")).transpose()?;
                    let announced = match (announced_at, &published) {
                        (Some(d), _) => Some(NaiveDate::parse_from_str(&d, "%Y-%m-%d")?),
                        (None, Some(d)) => Some(*d),
                        _ => None
                    };
                    
                    let budget_details = if team.is_some() || amounts.is_some() {
                        Some(BudgetRequestDetailsCommand {
                            team,
                            request_amounts: amounts.map(|a| parse_amounts(&a).unwrap()), //TODO remove the unwrap
                            start_date: start.map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d")).transpose()?,
                            end_date: end.map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d")).transpose()?,
                            is_loan: loan,
                            payment_address: address,
                        })
                    } else {
                        None
                    };

                    Ok(Command::AddProposal {
                        title,
                        url,
                        budget_request_details: budget_details,
                        announced_at: announced,
                        published_at: published,
                        is_historical: None,
                    })
                },
                ProposalCommands::Close { name, resolution } => {
                    Ok(Command::CloseProposal { proposal_name: name, resolution })
                },
                ProposalCommands::Update { 
                    name, title, url, team, amounts, start, end, loan, address, announced_at, published_at 
                } => {
                    let published = published_at.map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d")).transpose()?;
                    let announced = announced_at.map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d")).transpose()?;
        

                    let budget_details = if team.is_some() || amounts.is_some() {
                        Some(BudgetRequestDetailsCommand {
                            team,
                            request_amounts: amounts.map(|a| parse_amounts(&a).unwrap()), //TODO remove unwrap
                            start_date: start.map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d")).transpose()?,
                            end_date: end.map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d")).transpose()?,
                            is_loan: loan,
                            payment_address: address,
                        })
                    } else {
                        None
                    };

                    Ok(Command::UpdateProposal {
                        proposal_name: name,
                        updates: UpdateProposalDetails {
                            title,
                            url,
                            budget_request_details: budget_details,
                            announced_at: announced,
                            published_at: published,
                            resolved_at: None,
                        }
                    })
                },
                ProposalCommands::Pay { proposals, tx, date } => {
                    let payment_date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")?;
                    let proposal_names = proposals.split(',').map(String::from).collect();
                    Ok(Command::LogPayment {
                        payment_tx: tx,
                        payment_date,
                        proposal_names,
                    })
                }
            },

            Commands::Vote { command } => match command {
                VoteCommands::Process { name, counted, uncounted, opened, closed } => {
                    Ok(Command::CreateAndProcessVote {
                        proposal_name: name,
                        counted_votes: parse_votes(&counted)?,
                        uncounted_votes: parse_votes(&uncounted)?,
                        vote_opened: opened.map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d")).transpose()?,
                        vote_closed: closed.map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d")).transpose()?,
                    })
                }
            },

            Commands::Raffle { command } => match command {
                RaffleCommands::Create { name, block_offset, excluded } => {
                    Ok(Command::CreateRaffle {
                        proposal_name: name,
                        block_offset,
                        excluded_teams: excluded.map(|e| e.split(',').map(String::from).collect()),
                    })
                }
            },

            Commands::Report { command } => match command {
                ReportCommands::Team { team_name } => {
                    Ok(Command::PrintTeamReport { team_name })
                },
                ReportCommands::EpochState => {
                    Ok(Command::PrintEpochState)
                },
                ReportCommands::TeamParticipation { team_name, epoch_name } => {
                    Ok(Command::PrintTeamVoteParticipation { team_name, epoch_name })
                },
                ReportCommands::Points { epoch_name } => {
                    Ok(Command::PrintPointReport { epoch_name })
                },
                ReportCommands::EndOfEpoch { epoch_name } => {
                    Ok(Command::GenerateEndOfEpochReport { epoch_name })
                },
                ReportCommands::UnpaidRequests { output_path, epoch_name } => {
                    Ok(Command::GenerateUnpaidRequestsReport { output_path, epoch_name })
                },
                ReportCommands::EpochPayments { epoch_name, output } => {
                    Ok(Command::GenerateEpochPaymentsReport { 
                        epoch_name, 
                        output_path: output 
                    })
                },
                ReportCommands::ForProposal { proposal_name } => {
                    Ok(Command::GenerateReportForProposal { proposal_name })
                },
                ReportCommands::ClosedProposals { epoch_name } => {
                    Ok(Command::GenerateReportsForClosedProposals { epoch_name })
                },
                ReportCommands::AllEpochs { output_path, only_closed } => {
                    Ok(Command::GenerateAllEpochsReport { output_path, only_closed })
                },
            },

            Commands::Import { command } => match command {
                ImportCommands::PredefinedRaffle { 
                    proposal_name, 
                    counted_teams,
                    uncounted_teams,
                    total_counted_seats,
                    max_earner_seats 
                } => {
                    Ok(Command::ImportPredefinedRaffle {
                        proposal_name,
                        counted_teams,
                        uncounted_teams, 
                        total_counted_seats,
                        max_earner_seats
                    })
                },
                ImportCommands::HistoricalVote {
                    proposal_name,
                    passed,
                    participating_teams,
                    non_participating_teams,
                    counted_points,
                    uncounted_points
                } => {
                    Ok(Command::ImportHistoricalVote {
                        proposal_name,
                        passed,
                        participating_teams,
                        non_participating_teams,
                        counted_points,
                        uncounted_points
                    })
                },
                ImportCommands::HistoricalRaffle {
                    proposal_name,
                    initiation_block,
                    randomness_block,
                    team_order,
                    excluded_teams,
                    total_counted_seats,
                    max_earner_seats
                } => {
                    Ok(Command::ImportHistoricalRaffle {
                        proposal_name,
                        initiation_block,
                        randomness_block,
                        team_order,
                        excluded_teams,
                        total_counted_seats,
                        max_earner_seats
                    })
                }
            },

            Commands::RunScript { script_file_path } => {
                Ok(Command::RunScript { script_file_path })
            },
        }
    }
}


pub async fn execute_command<W: Write + Send + 'static>(
    budget_system: &mut BudgetSystem,
    command: Command,
    config: &AppConfig,
    output: &mut W
) -> Result<(), Box<dyn Error>> {
    match command {
        Command::RunScript { script_file_path } => {
            let script_path = script_file_path.unwrap_or_else(|| config.script_file.clone());
            let script_commands = read_script_commands(&script_path)?;
            for cmd in script_commands {
                budget_system.execute_command_with_streaming(cmd, output).await?;
            }
            Ok(())
        },
        _ => {
            budget_system.execute_command_with_streaming(command, output).await
        }
    }
}

pub fn parse_cli_args(args: &[String]) -> Result<Command, Box<dyn Error>> {
    let cli = Cli::parse_from(args);
    cli.into_command()
}

fn parse_amounts(amounts_str: &str) -> Result<HashMap<String, f64>, Box<dyn Error>> {
    amounts_str
        .split(',')
        .map(|pair| {
            let parts: Vec<&str> = pair.split(':').collect();
            if parts.len() != 2 {
                return Err("Invalid amount format. Expected token:amount".into());
            }
            let amount = parts[1].parse::<f64>()
                .map_err(|_| format!("Invalid amount: {}", parts[1]))?;
            Ok((parts[0].to_string(), amount))
        })
        .collect()
}

pub fn read_script_commands(script_file_path: &str) -> Result<Vec<Command>, Box<dyn Error>> {
    let script_content = fs::read_to_string(script_file_path)?;
    let commands: Vec<Command> = serde_json::from_str(&script_content)?;
    Ok(commands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};

    // Helper function to convert string args into Vec<String>
    fn args(args: &[&str]) -> Vec<String> {
        std::iter::once("robokitty".to_string())
            .chain(args.iter().map(|&s| s.to_string()))
            .collect()
    }

    // Helper to parse date string to DateTime<Utc>
    fn parse_date(date_str: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(date_str)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn test_team_add_command_full() {
        let args = args(&[
            "team", 
            "add",
            "--name", "Engineering",
            "--representative", "Alice",
            "--revenue", "1000,2000,3000",
            "--address", "0x1234567890123456789012345678901234567890"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::AddTeam { 
                name, 
                representative, 
                trailing_monthly_revenue, 
                address 
            } => {
                assert_eq!(name, "Engineering");
                assert_eq!(representative, "Alice");
                assert_eq!(trailing_monthly_revenue, Some(vec![1000, 2000, 3000]));
                assert_eq!(address, Some("0x1234567890123456789012345678901234567890".to_string()));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_team_add_command_minimal() {
        let args = args(&[
            "team",
            "add",
            "--name", "Engineering",
            "--representative", "Alice"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::AddTeam { 
                name, 
                representative, 
                trailing_monthly_revenue, 
                address 
            } => {
                assert_eq!(name, "Engineering");
                assert_eq!(representative, "Alice");
                assert_eq!(trailing_monthly_revenue, None);
                assert_eq!(address, None);
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_team_update_command_full() {
        let args = args(&[
            "team",
            "update",
            "Engineering",
            "--new-name", "Engineering Team",
            "--representative", "Bob",
            "--status", "Earner",
            "--revenue", "2000,3000,4000",
            "--address", "0x1234567890123456789012345678901234567890"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::UpdateTeam { team_name, updates } => {
                assert_eq!(team_name, "Engineering");
                assert_eq!(updates.name, Some("Engineering Team".to_string()));
                assert_eq!(updates.representative, Some("Bob".to_string()));
                assert_eq!(updates.status, Some("Earner".to_string()));
                assert_eq!(updates.trailing_monthly_revenue, Some(vec![2000, 3000, 4000]));
                assert_eq!(updates.address, Some("0x1234567890123456789012345678901234567890".to_string()));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_team_update_command_partial() {
        let args = args(&[
            "team",
            "update",
            "Engineering",
            "--new-name", "Engineering Team"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::UpdateTeam { team_name, updates } => {
                assert_eq!(team_name, "Engineering");
                assert_eq!(updates.name, Some("Engineering Team".to_string()));
                assert_eq!(updates.representative, None);
                assert_eq!(updates.status, None);
                assert_eq!(updates.trailing_monthly_revenue, None);
                assert_eq!(updates.address, None);
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_team_add_invalid_revenue() {
        let args = args(&[
            "team",
            "add",
            "--name", "Engineering",
            "--representative", "Alice",
            "--revenue", "abc,def"
        ]);

        let result = parse_cli_args(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_team_add_invalid_address() {
        let args = args(&[
            "team",
            "add",
            "--name", "Engineering",
            "--representative", "Alice",
            "--address", "not_a_hex_address"
        ]);

        let result = parse_cli_args(&args);
        assert!(result.is_err());
        // Optional: verify error message
        assert!(matches!(result,
            Err(ref e) if e.to_string().contains("address")));
    }

    #[test]
    fn test_epoch_create_command() {
        let args = args(&[
            "epoch",
            "create",
            "Q1-2024",
            "2024-01-01T00:00:00Z",
            "2024-03-31T23:59:59Z"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::CreateEpoch { name, start_date, end_date } => {
                assert_eq!(name, "Q1-2024");
                assert_eq!(start_date, parse_date("2024-01-01T00:00:00Z"));
                assert_eq!(end_date, parse_date("2024-03-31T23:59:59Z"));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_epoch_activate_command() {
        let args = args(&[
            "epoch",
            "activate",
            "Q1-2024"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::ActivateEpoch { name } => {
                assert_eq!(name, "Q1-2024");
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_epoch_set_reward_command() {
        let args = args(&[
            "epoch",
            "set-reward",
            "ETH",
            "100.5"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::SetEpochReward { token, amount } => {
                assert_eq!(token, "ETH");
                assert_eq!(amount, 100.5);
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_epoch_close_command() {
        let args = args(&[
            "epoch",
            "close",
            "Q1-2024"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::CloseEpoch { epoch_name } => {
                assert_eq!(epoch_name, Some("Q1-2024".to_string()));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_epoch_create_invalid_dates() {
        let args = args(&[
            "epoch",
            "create",
            "Q1-2024",
            "invalid-date",
            "2024-03-31T23:59:59Z"
        ]);

        assert!(parse_cli_args(&args).is_err());
    }

    #[test]
    fn test_epoch_set_reward_negative_amount() {
        let args = args(&[
            "epoch",
            "set-reward",
            "ETH",
            "--",  // Add this to indicate end of options
            "-100.5"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::SetEpochReward { token, amount } => {
                assert_eq!(token, "ETH");
                assert_eq!(amount, -100.5);
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_epoch_create_end_before_start() {
        let args = args(&[
            "epoch",
            "create",
            "Q1-2024",
            "2024-03-31T23:59:59Z",  // End date first
            "2024-01-01T00:00:00Z"   // Start date second
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        // Note: The current implementation doesn't validate date order
        // You might want to add this validation
        match cmd {
            Command::CreateEpoch { name, start_date, end_date } => {
                assert_eq!(name, "Q1-2024");
                assert_eq!(start_date, parse_date("2024-03-31T23:59:59Z"));
                assert_eq!(end_date, parse_date("2024-01-01T00:00:00Z"));
            },
            _ => panic!("Wrong command type"),
        }
    }

    // Additional test helpers
    fn valid_eth_address() -> String {
        "0x1234567890123456789012345678901234567890".to_string()
    }

    // Proposal Command Tests
    #[test]
    fn test_proposal_add_command_full() {
        let args = args(&[
            "proposal", 
            "add",
            "--title", "Test Proposal",
            "--url", "https://example.com",
            "--team", "Engineering",
            "--amounts", "ETH:100.5,USD:1000",
            "--start", "2024-01-01",
            "--end", "2024-12-31",
            "--loan", "true",
            "--address", &valid_eth_address()
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::AddProposal { 
                title, 
                url, 
                budget_request_details,
                announced_at,
                published_at,
                is_historical,
            } => {
                assert_eq!(title, "Test Proposal");
                assert_eq!(url, Some("https://example.com".to_string()));
                
                let details = budget_request_details.unwrap();
                assert_eq!(details.team, Some("Engineering".to_string()));
                assert_eq!(details.request_amounts.unwrap().get("ETH").unwrap(), &100.5);
                assert_eq!(details.start_date.unwrap(), NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
                assert_eq!(details.end_date.unwrap(), NaiveDate::from_ymd_opt(2024, 12, 31).unwrap());
                assert_eq!(details.is_loan, Some(true));
                assert_eq!(details.payment_address, Some(valid_eth_address()));
                
                assert_eq!(announced_at, None);
                assert_eq!(published_at, None);
                assert_eq!(is_historical, None);
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_proposal_add_command_loan_flags() {
        // Test with loan true and team (to ensure budget_request_details is created)
        let args_loan_true = args(&[
            "proposal", 
            "add",
            "--title", "Test Proposal",
            "--team", "Engineering", // Added team to ensure budget_request_details is created
            "--loan", "true"
        ]);

        let cmd_loan_true = parse_cli_args(&args_loan_true).unwrap();
        match cmd_loan_true {
            Command::AddProposal { budget_request_details, .. } => {
                let details = budget_request_details.unwrap();
                assert_eq!(details.is_loan, Some(true));
            },
            _ => panic!("Wrong command type"),
        }

        // Test with loan false
        let args_loan_false = args(&[
            "proposal", 
            "add",
            "--title", "Test Proposal",
            "--team", "Engineering",
            "--loan", "false"
        ]);

        let cmd_loan_false = parse_cli_args(&args_loan_false).unwrap();
        match cmd_loan_false {
            Command::AddProposal { budget_request_details, .. } => {
                let details = budget_request_details.unwrap();
                assert_eq!(details.is_loan, Some(false));
            },
            _ => panic!("Wrong command type"),
        }

        // Test without loan flag
        let args_no_loan = args(&[
            "proposal", 
            "add",
            "--title", "Test Proposal"
        ]);

        let cmd_no_loan = parse_cli_args(&args_no_loan).unwrap();
        match cmd_no_loan {
            Command::AddProposal { budget_request_details, .. } => {
                assert!(budget_request_details.is_none());
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_proposal_add_command_minimal() {
        let args = args(&[
            "proposal",
            "add",
            "--title", "Test Proposal"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::AddProposal { 
                title, 
                url, 
                budget_request_details,
                ..
            } => {
                assert_eq!(title, "Test Proposal");
                assert_eq!(url, None);
                assert_eq!(budget_request_details, None);
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_proposal_update_command() {
        let args = args(&[
            "proposal",
            "update",
            "test-proposal",
            "--title", "Updated Title",
            "--url", "https://new.example.com",
            "--team", "NewTeam",
            "--amounts", "ETH:200.5",
            "--loan", "true"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::UpdateProposal { 
                proposal_name, 
                updates 
            } => {
                assert_eq!(proposal_name, "test-proposal");
                assert_eq!(updates.title, Some("Updated Title".to_string()));
                assert_eq!(updates.url, Some("https://new.example.com".to_string()));
                
                let details = updates.budget_request_details.unwrap();
                assert_eq!(details.team, Some("NewTeam".to_string()));
                assert_eq!(details.request_amounts.unwrap().get("ETH").unwrap(), &200.5);
                assert_eq!(details.is_loan, Some(true));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_proposal_close_command() {
        let args = args(&[
            "proposal",
            "close",
            "test-proposal",
            "Approved"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::CloseProposal { 
                proposal_name, 
                resolution 
            } => {
                assert_eq!(proposal_name, "test-proposal");
                assert_eq!(resolution, "Approved");
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_proposal_add_invalid_dates() {
        let args = args(&[
            "proposal",
            "add",
            "--title", "Test Proposal",
            "--team", "Engineering",
            "--start", "2024-13-45", // Invalid month and day
            "--end", "2024-12-31"
        ]);

        let result = parse_cli_args(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_proposal_close_invalid_resolution() {
        let args = args(&[
            "proposal",
            "close",
            "test-proposal",
            "InvalidResolution"
        ]);

        // Note: Current implementation doesn't validate resolution string
        // You might want to add this validation
        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::CloseProposal { resolution, .. } => {
                assert_eq!(resolution, "InvalidResolution");
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_parse_amounts_valid() {
        let result = parse_amounts("ETH:100.5,USD:1000").unwrap();
        assert_eq!(result.get("ETH").unwrap(), &100.5);
        assert_eq!(result.get("USD").unwrap(), &1000.0);
    }

    #[test]
    fn test_parse_amounts_invalid() {
        let result = parse_amounts("ETH:not_a_number");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid amount: not_a_number"));
    }

    #[test]
    fn test_parse_amounts_invalid_format() {
        let result = parse_amounts("invalid_format");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid amount format"));
    }

    #[test]
    fn test_proposal_update_valid_amounts() {
        let args = args(&[
            "proposal",
            "update",
            "test-proposal",
            "--amounts", "ETH:100.5,USD:1000"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::UpdateProposal { updates, .. } => {
                let amounts = updates
                    .budget_request_details
                    .unwrap()
                    .request_amounts
                    .unwrap();
                assert_eq!(amounts.get("ETH").unwrap(), &100.5);
                assert_eq!(amounts.get("USD").unwrap(), &1000.0);
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    #[should_panic(expected = "Invalid amount: not_a_number")]
    fn test_proposal_update_invalid_amounts() {
        let args = args(&[
            "proposal",
            "update",
            "test-proposal",
            "--amounts", "ETH:not_a_number"
        ]);

        // This will panic due to the unwrap in the code
        let _ = parse_cli_args(&args);
    }

    // Vote Command Tests
    #[test]
    fn test_vote_process_command_full() {
        let args = args(&[
            "vote",
            "process",
            "test-proposal",
            "--counted", "Team1:Yes,Team2:No",
            "--uncounted", "Team3:Yes",
            "--opened", "2024-01-01",
            "--closed", "2024-01-07"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::CreateAndProcessVote { 
                proposal_name,
                counted_votes,
                uncounted_votes,
                vote_opened,
                vote_closed,
            } => {
                assert_eq!(proposal_name, "test-proposal");
                
                assert_eq!(counted_votes.len(), 2);
                assert_eq!(counted_votes.get("Team1").unwrap(), &VoteChoice::Yes);
                assert_eq!(counted_votes.get("Team2").unwrap(), &VoteChoice::No);
                
                assert_eq!(uncounted_votes.len(), 1);
                assert_eq!(uncounted_votes.get("Team3").unwrap(), &VoteChoice::Yes);
                
                assert_eq!(vote_opened.unwrap(), NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
                assert_eq!(vote_closed.unwrap(), NaiveDate::from_ymd_opt(2024, 1, 7).unwrap());
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_vote_process_command_minimal() {
        let args = args(&[
            "vote",
            "process",
            "test-proposal",
            "--counted", "Team1:Yes",
            "--uncounted", "Team2:No"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::CreateAndProcessVote { 
                proposal_name,
                counted_votes,
                uncounted_votes,
                vote_opened,
                vote_closed,
            } => {
                assert_eq!(proposal_name, "test-proposal");
                assert_eq!(counted_votes.len(), 1);
                assert_eq!(uncounted_votes.len(), 1);
                assert!(vote_opened.is_none());
                assert!(vote_closed.is_none());
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_vote_process_invalid_format() {
        let args = args(&[
            "vote",
            "process",
            "test-proposal",
            "--counted", "invalid_format",
            "--uncounted", "Team2:No"
        ]);

        assert!(parse_cli_args(&args).is_err());
    }

    #[test]
    fn test_vote_process_invalid_dates() {
        let args = args(&[
            "vote",
            "process",
            "test-proposal",
            "--counted", "Team1:Yes",
            "--uncounted", "Team2:No",
            "--opened", "invalid-date"
        ]);

        assert!(parse_cli_args(&args).is_err());
    }

    #[test]
    fn test_vote_process_duplicate_team() {
        let args = args(&[
            "vote",
            "process",
            "test-proposal",
            "--counted", "Team1:Yes,Team1:No",
            "--uncounted", "Team2:No"
        ]);

        // Note: Current implementation allows duplicate teams by overwriting
        // You might want to add validation to prevent this
        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::CreateAndProcessVote { counted_votes, .. } => {
                assert_eq!(counted_votes.len(), 1);
                assert_eq!(counted_votes.get("Team1").unwrap(), &VoteChoice::No);
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_raffle_create_command_full() {
        let args = args(&[
            "raffle",
            "create",
            "test-proposal",
            "--block-offset", "100",
            "--excluded", "Team1,Team2,Team3"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::CreateRaffle { 
                proposal_name,
                block_offset,
                excluded_teams,
            } => {
                assert_eq!(proposal_name, "test-proposal");
                assert_eq!(block_offset, Some(100));
                assert_eq!(excluded_teams, Some(vec!["Team1".to_string(), "Team2".to_string(), "Team3".to_string()]));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_raffle_create_command_minimal() {
        let args = args(&[
            "raffle",
            "create",
            "test-proposal"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        
        match cmd {
            Command::CreateRaffle { 
                proposal_name,
                block_offset,
                excluded_teams,
            } => {
                assert_eq!(proposal_name, "test-proposal");
                assert_eq!(block_offset, None);
                assert_eq!(excluded_teams, None);
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_raffle_create_duplicate_excluded_teams() {
        let args = args(&[
            "raffle",
            "create", 
            "test-proposal",
            "--excluded", "Team1,Team1,Team2"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::CreateRaffle { excluded_teams, .. } => {
                let teams = excluded_teams.unwrap();
                // Note: Current implementation allows duplicates
                assert_eq!(teams.len(), 3);
                assert_eq!(teams, vec!["Team1".to_string(), "Team1".to_string(), "Team2".to_string()]);
            },
            _ => panic!("Wrong command type"),
        }
    }

    // Report Command Tests
    #[test]
    fn test_report_team_command() {
        let args = args(&["report", "team"]);
        let cmd = parse_cli_args(&args).unwrap();
        assert!(matches!(cmd, Command::PrintTeamReport { team_name: None }));
    }

    #[test]
    fn test_report_team_command_with_team_name() {
        let args = args(&["report", "team", "Dinobots"]);
        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::PrintTeamReport { team_name } => {
                assert_eq!(team_name, Some("Dinobots".to_string()));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_report_epoch_state_command() {
        let args = args(&["report", "epoch-state"]);
        let cmd = parse_cli_args(&args).unwrap();
        assert!(matches!(cmd, Command::PrintEpochState));
    }

    #[test]
    fn test_report_team_participation_command() {
        let args = args(&[
            "report", 
            "team-participation", 
            "Engineering",
            "Q1-2024"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::PrintTeamVoteParticipation { team_name, epoch_name } => {
                assert_eq!(team_name, "Engineering");
                assert_eq!(epoch_name, Some("Q1-2024".to_string()));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_report_points_command() {
        let args = args(&[
            "report", 
            "points",
            "--epoch-name", "Q1-2024"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::PrintPointReport { epoch_name } => {
                assert_eq!(epoch_name, Some("Q1-2024".to_string()));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_report_closed_proposals_command() {
        let args = args(&[
            "report",
            "closed-proposals",
            "Q1-2024"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::GenerateReportsForClosedProposals { epoch_name } => {
                assert_eq!(epoch_name, "Q1-2024");
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_report_end_of_epoch_command() {
        let args = args(&[
            "report",
            "end-of-epoch",
            "Q1-2024"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::GenerateEndOfEpochReport { epoch_name } => {
                assert_eq!(epoch_name, "Q1-2024");
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_report_unpaid_requests_command() {
        let args = args(&[
            "report",
            "unpaid-requests",
            "--output-path", "/tmp/report.txt",
            "--epoch-name", "Q1-2024"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::GenerateUnpaidRequestsReport { output_path, epoch_name } => {
                assert_eq!(output_path, Some("/tmp/report.txt".to_string()));
                assert_eq!(epoch_name, Some("Q1-2024".to_string()));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_report_for_proposal_command() {
        let args = args(&[
            "report",
            "for-proposal",
            "test-proposal"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::GenerateReportForProposal { proposal_name } => {
                assert_eq!(proposal_name, "test-proposal");
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_proposal_add_with_dates() {
        let args = args(&[
            "proposal", 
            "add",
            "--title", "Test Proposal",
            "--announced-at", "2024-01-01",
            "--published-at", "2024-01-15"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::AddProposal { announced_at, published_at, .. } => {
                assert_eq!(announced_at, Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()));
                assert_eq!(published_at, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_proposal_add_published_only() {
        let args = args(&[
            "proposal", 
            "add",
            "--title", "Test Proposal",
            "--published-at", "2024-01-15"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::AddProposal { announced_at, published_at, .. } => {
                assert_eq!(announced_at, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
                assert_eq!(published_at, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_proposal_update_with_dates() {
        let args = args(&[
            "proposal",
            "update",
            "test-proposal",
            "--announced-at", "2024-01-01",
            "--published-at", "2024-01-15"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::UpdateProposal { updates, .. } => {
                assert_eq!(updates.announced_at, Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()));
                assert_eq!(updates.published_at, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_proposal_update_published_only() {
        let args = args(&[
            "proposal",
            "update", 
            "test-proposal",
            "--published-at", "2024-01-15"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::UpdateProposal { updates, .. } => {
                assert_eq!(updates.announced_at, None);
                assert_eq!(updates.published_at, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_proposal_invalid_dates() {
        let args = args(&[
            "proposal",
            "add",
            "--title", "Test Proposal", 
            "--announced-at", "invalid-date"
        ]);

        assert!(parse_cli_args(&args).is_err());
    }

    #[test]
    fn test_proposal_pay_command() {
    let args = args(&[
        "proposal", 
        "pay",
        "proposal1,proposal2",
        "--tx", "0x742d35Cc6634C0532925a3b844Bc454e4438f44e4438f44e4438f44e4438f44e",
        "--date", "2024-01-01"
    ]);

    let cmd = parse_cli_args(&args).unwrap();
    match cmd {
        Command::LogPayment { payment_tx, payment_date, proposal_names } => {
            assert_eq!(payment_tx, "0x742d35Cc6634C0532925a3b844Bc454e4438f44e4438f44e4438f44e4438f44e");
            assert_eq!(payment_date, NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
            assert_eq!(proposal_names, vec!["proposal1", "proposal2"]);
        },
        _ => panic!("Wrong command type"),
    }
    }

    #[test]
    fn test_proposal_pay_invalid_date() {
    let args = args(&[
        "proposal",
        "pay", 
        "proposal1",
        "--tx", "0x742d35Cc6634C0532925a3b844Bc454e4438f44e4438f44e4438f44e4438f44e",
        "--date", "invalid-date"
    ]);

    assert!(parse_cli_args(&args).is_err());
    }

    #[test]
    fn test_epoch_payments_command() {
        let args = args(&[
            "report",
            "epoch-payments",
            "Q1-2024",
            "--output", "payments.json"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::GenerateEpochPaymentsReport { epoch_name, output_path } => {
                assert_eq!(epoch_name, "Q1-2024");
                assert_eq!(output_path, Some("payments.json".to_string()));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_epoch_payments_command_no_output() {
        let args = args(&[
            "report",
            "epoch-payments",
            "Q1-2024"
        ]);

        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::GenerateEpochPaymentsReport { epoch_name, output_path } => {
                assert_eq!(epoch_name, "Q1-2024");
                assert_eq!(output_path, None);
            },
            _ => panic!("Wrong command type"),
        }
    }

}

// TODO: Missing unit tests for CLI
//
// # CLI Argument Parsing Tests

// ## Import Command Tests
// * Should successfully parse predefined raffle import command
// * Should successfully parse historical vote import command
// * Should successfully parse historical raffle import command
// * Should reject predefined raffle import with invalid seat counts
// * Should reject historical vote import with invalid team lists
// * Should reject historical raffle import with invalid block numbers

// # Utility Function Tests

// ## Ethereum Address Parsing
// * Should successfully parse valid Ethereum address
// * Should reject address without 0x prefix
// * Should reject address with invalid length
// * Should reject address with invalid characters
// * Should reject empty address

// ## Vote Parsing
// * Should successfully parse valid yes votes
// * Should successfully parse valid no votes
// * Should successfully parse mixed yes/no votes
// * Should reject invalid vote choice values
// * Should reject malformed vote strings
// * Should reject empty vote string
// * Should reject duplicate team votes

// ## Amount Parsing
// * Should successfully parse single amount
// * Should successfully parse multiple amounts
// * Should successfully parse decimal amounts
// * Should reject invalid amount format
// * Should reject negative amounts
// * Should reject malformed amount strings
// * Should reject duplicate token entries

// # Integration Tests

// ## Script Processing
// * Should successfully read and parse valid script file
// * Should successfully execute multiple commands from script
// * Should handle empty script file
// * Should reject malformed JSON script
// * Should reject script with invalid commands
// * Should maintain command order during execution

// ## Command Execution Flow
// * Should successfully execute command sequence
// * Should maintain system state across commands
// * Should properly handle command dependencies
// * Should rollback on command failure
// * Should handle concurrent command execution

// # Error Handling Tests

// ## Input Validation
// * Should provide clear error for missing required fields
// * Should provide clear error for invalid date formats
// * Should provide clear error for invalid numeric values
// * Should provide clear error for invalid enum values
// * Should handle UTF-8 encoding issues in input

// ## System State Validation
// * Should detect and handle duplicate team names
// * Should detect and handle invalid epoch transitions
// * Should detect and handle invalid proposal states
// * Should detect and handle invalid vote states
// * Should enforce proper command sequencing

// ## Resource Handling
// * Should properly handle file system errors
// * Should properly handle I/O errors
// * Should handle large input files
// * Should handle concurrent access to resources
// * Should cleanup resources on error
