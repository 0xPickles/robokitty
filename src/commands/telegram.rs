use teloxide::utils::command::BotCommands;
use crate::escape_markdown;
use crate::core::budget_system::BudgetSystem;
use crate::core::models::VoteChoice;
use crate::commands::common::{Command, CommandExecutor, BudgetRequestDetailsCommand, UpdateProposalDetails, UpdateTeamDetails};
use chrono::{NaiveDate, DateTime, Utc, TimeZone};
use std::collections::HashMap;

/// These commands are supported:
#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "snake_case",
)]
pub enum TelegramCommand {
    /// Display this text.
    /// 
    Help,
    
    /// Display team information.
    /// 
    PrintTeamReport,
    
    /// Show current epoch status.
    /// 
    PrintEpochState,
    
    /// Activate an epoch. Usage: /activate_epoch <name>
    /// 
    #[command(parse_with = "split")]
    ActivateEpoch {
        name: String
    },

    /// Set epoch reward. Usage: /set_epoch_reward <token> <amount>
    /// 
    #[command(parse_with = "split")]
    SetEpochReward {
        token: String,
        amount: String
    },

    /// Display a team's vote participation. Usage: /print_team_participation <team_name> <epoch_name>
    /// 
    #[command(parse_with = "split")]
    PrintTeamParticipation{
        team_name: String,
        epoch_name: String
    },

    /// Create a new epoch. Usage: /create_epoch <name> <start_date YYYY-MM-DD> <end_date YYYY-MM-DD>
    /// 
    #[command(parse_with = "split")]
    CreateEpoch{
        name: String,
        start_date: String,
        end_date: String
    },

    /// Add a new team. 
    /// Usage: /add_team name:TeamName rep:Representative [rev:1000,2000,3000]
    /// For supporter teams, omit the rev parameter
    /// 
    AddTeam {
        args: String,
    },

    /// Update a team's details. 
    /// Usage: /update_team team:TeamName [name:NewName] [rep:NewRep] [status:Earner|Supporter|Inactive] [rev:1000,2000,3000]
    /// Note: Earner status requires revenue data
    /// 
        UpdateTeam {
        args: String,
    },

    /// Add a new proposal. 
    /// Usage: /add_proposal title:ProposalTitle url:https://example.com [team:TeamName] [amounts:ETH:100.5,USD:1000] [start:2024-01-01] [end:2024-12-31] [announced:2024-01-01] [published:2024-01-01] [loan:true/false] [address:0x...]
    /// 
    AddProposal {
        args: String,
    },

    /// Update a proposal's details. 
    /// Usage: /update_proposal proposal:ExistingTitle [title:NewTitle] [url:NewURL] [team:TeamName] [amounts:ETH:200.5,USD:2000] [start:2024-02-01] [end:2024-12-31] [announced:2024-01-01] [published:2024-01-01] [resolved:2024-12-31]
    /// 
    UpdateProposal {
        args: String,
    },

    /// Close a proposal with resolution. 
    /// Usage: /close_proposal name:ProposalName res:Resolution
    /// 
    CloseProposal {
        args: String,
    },

    /// Process a vote for a proposal.
    /// Usage: /process_vote name:ProposalName counted:Team1:Yes,Team2:No uncounted:Team3:Yes,Team4:No opened:2024-01-01 closed:2024-01-01
    /// 
    ProcessVote {
        args: String,
    },

    /// Create a raffle for a proposal. 
    /// Usage: /create_raffle name:ProposalName [block_offset:10] [excluded:Team1,Team2]
    /// 
    CreateRaffle {
        args: String,
    },

    /// Generate unpaid requests report. 
    /// Usage: /generate_unpaid_report [epoch_name]
    GenerateUnpaidReport {
        args: String,
    },

    /// Generate epoch payments report.
    /// Usage: /epoch_payments <epoch_name>
    EpochPayments {
        epoch_name: String,
    },

    /// Log payment for proposals.
    /// Usage: /log_payment tx:<HASH> date:<YYYY-MM-DD> proposals:<PROP1,PROP2,...>
    LogPayment {
        args: String, 
    }

}

#[derive(Debug)]
struct AddTeamArgs {
    name: String,
    representative: String,
    revenue: Option<Vec<u64>>,
    address: Option<String>,
}

#[derive(Debug)]
struct UpdateTeamArgs {
    team: String,
    new_name: Option<String>,
    representative: Option<String>,
    status: Option<String>,
    revenue: Option<Vec<u64>>,
    address: Option<String>,
}

#[derive(Debug)]
struct AddProposalArgs {
    title: String,
    url: String,
    team: Option<String>,
    amounts: Option<HashMap<String, f64>>,
    start_date: Option<String>,
    end_date: Option<String>,
    announced_date: Option<String>,
    published_date: Option<String>,
    is_loan: Option<bool>,
    payment_address: Option<String>,
}

#[derive(Debug)]
struct UpdateProposalArgs {
    proposal_name: String,
    new_title: Option<String>,
    url: Option<String>,
    team: Option<String>,
    amounts: Option<HashMap<String, f64>>,
    start_date: Option<String>,
    end_date: Option<String>,
    announced_date: Option<String>,
    published_date: Option<String>,
    resolved_date: Option<String>,
    is_loan: Option<bool>,
    payment_address: Option<String>,
}

#[derive(Debug)]
struct CloseProposalArgs {
    name: String,
    resolution: String,
}

#[derive(Debug)]
struct ProcessVoteArgs {
    name: String,
    counted_votes: HashMap<String, VoteChoice>,
    uncounted_votes: HashMap<String, VoteChoice>,
    vote_opened: Option<NaiveDate>,
    vote_closed: Option<NaiveDate>,
}

#[derive(Debug)]
struct CreateRaffleArgs {
    proposal_name: String,
    block_offset: Option<u64>,
    excluded_teams: Option<Vec<String>>,
}

impl TelegramCommand {
    fn parse_date(date_str: &str) -> Result<NaiveDate, String> {
        NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|e| format!("Invalid date format (use YYYY-MM-DD): {}", e))
    }
    
    fn parse_start_date(date_str: &str) -> Result<DateTime<Utc>, String> {
        let date = Self::parse_date(date_str)?;
        Ok(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()))
    }
    
    fn parse_end_date(date_str: &str) -> Result<DateTime<Utc>, String> {
        let date = Self::parse_date(date_str)?;
        Ok(Utc.from_utc_datetime(&date.and_hms_opt(23, 59, 59).unwrap()))
    }
    
    fn parse_command(input: &str) -> Result<Vec<String>, String> {
        let mut args = Vec::new();
        let mut current_arg = String::new();
        
        for part in input.split_whitespace() {
            if part.contains(':') {
                // If we have a pending arg, push it
                if !current_arg.is_empty() {
                    args.push(current_arg);
                }
                current_arg = part.to_string();
            } else if !current_arg.is_empty() {
                // Append to current arg
                current_arg.push(' ');
                current_arg.push_str(part);
            } else {
                return Err(format!("Invalid argument format: {}. Expected key:value", part));
            }
        }

        // Push final arg if exists
        if !current_arg.is_empty() {
            args.push(current_arg);
        }
        
        Ok(args)
    }

    fn parse_add_team(args: &[String]) -> Result<AddTeamArgs, String> {
        if args.len() < 2 {
            return Err("Usage: /add_team name:<name> rep:<representative> rev:<revenue1,revenue2,revenue3> addy:<default_payment_address>".to_string());
        }

        let mut name = None;
        let mut representative = None;
        let mut revenue = None;
        let mut address = None;

        for arg in args {
            if let Some((key, value)) = arg.split_once(':') {
                match key {
                    "name" => name = Some(value.to_string()),
                    "rep" => representative = Some(value.to_string()),
                    "rev" => {
                        revenue = Some(value.split(',')
                            .map(|v| v.parse::<u64>())
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(|e| format!("Invalid revenue format: {}", e))?)
                    },
                    "addy" => address = Some(value.to_string()),
                    _ => return Err(format!("Unknown parameter: {}", key))
                }
            }
        }

        Ok(AddTeamArgs {
            name: name.ok_or("Missing name parameter")?,
            representative: representative.ok_or("Missing rep parameter")?,
            revenue,
            address,
        })
    }

    fn parse_update_team(args: &[String]) -> Result<UpdateTeamArgs, String> {
        let mut team = None;
        let mut new_name = None;
        let mut representative = None;
        let mut status = None;
        let mut revenue = None;
        let mut address = None;

        for arg in args {
            if let Some((key, value)) = arg.split_once(':') {
                match key {
                    "team" => team = Some(value.to_string()),
                    "name" => new_name = Some(value.to_string()),
                    "rep" => representative = Some(value.to_string()),
                    "status" => {
                        // Validate status here
                        match value.to_lowercase().as_str() {
                            "earner" | "supporter" | "inactive" => status = Some(value.to_string()),
                            _ => return Err(format!("Invalid status: {}. Must be one of: Earner, Supporter, Inactive", value))
                        }
                    },
                    "rev" => {
                        revenue = Some(value.split(',')
                            .map(|v| v.parse::<u64>())
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(|e| format!("Invalid revenue format: {}", e))?)
                    },
                    "address" => address = Some(value.to_string()),
                    _ => return Err(format!("Unknown parameter: {}", key))
                }
            }
        }

        Ok(UpdateTeamArgs {
            team: team.ok_or("Missing team parameter")?,
            new_name,
            representative,
            status,
            revenue,
            address
        })
    }

    fn parse_add_proposal(args: &[String]) -> Result<AddProposalArgs, String> {
        if args.len() < 2 {
            return Err("Usage: /add_proposal title:\"<title>\" url:\"<url>\" [team:<name>] [amounts:<token>:<amount>,...] [start:<YYYY-MM-DD>] [end:<YYYY-MM-DD>] [ann:<YYYY-MM-DD>] [pub:<YYYY-MM-DD>] [loan:true/false] [address:<eth_address>]".to_string());
        }

        let mut title = None;
        let mut url = None;
        let mut team = None;
        let mut amounts = None;
        let mut start_date = None;
        let mut end_date = None;
        let mut announced_date = None;
        let mut published_date = None;
        let mut is_loan = None;
        let mut payment_address = None;

        for arg in args {
            if let Some((key, value)) = arg.split_once(':') {
                match key {
                    "title" => title = Some(value.to_string()),
                    "url" => url = Some(value.to_string()),
                    "team" => team = Some(value.to_string()),
                    "amounts" => amounts = Some(Self::parse_amounts(value)?),
                    "start" => start_date = Some(value.to_string()),
                    "end" => end_date = Some(value.to_string()),
                    "announced" => announced_date = Some(value.to_string()),
                    "published" => published_date = Some(value.to_string()),
                    "loan" => {
                        is_loan = Some(value.parse::<bool>()
                            .map_err(|_| format!("Invalid loan value: {}", value))?);
                    },
                    "address" => payment_address = Some(value.to_string()),
                    _ => return Err(format!("Unknown parameter: {}", key))
                }
            }
        }

        Ok(AddProposalArgs {
            title: title.ok_or("Missing title parameter")?,
            url: url.ok_or("Missing url parameter")?,
            team,
            amounts,
            start_date,
            end_date,
            announced_date,
            published_date,
            is_loan,
            payment_address,
        })
    }

    fn parse_amounts(amounts_str: &str) -> Result<HashMap<String, f64>, String> {
        amounts_str.split(',')
            .map(|pair| {
                let parts: Vec<&str> = pair.split(':').collect();
                if parts.len() != 2 {
                    return Err(format!("Invalid amount format: {}. Expected token:amount", pair));
                }
                let amount = parts[1].parse::<f64>()
                    .map_err(|e| format!("Invalid amount {}: {}", parts[1], e))?;
                Ok((parts[0].to_string(), amount))
            })
            .collect()
    }

    fn parse_update_proposal(args: &[String]) -> Result<UpdateProposalArgs, String> {

        if args.is_empty() {
            return Err("Usage: /update_proposal proposal:\"Name\" [title:\"New Title\"] [url:\"new-url\"] \
                        [team:\"name\"] [amounts:\"token:amount\"] [start:\"YYYY-MM-DD\"] [end:\"YYYY-MM-DD\"] \
                        [announced:\"YYYY-MM-DD\"] [published:\"YYYY-MM-DD\"] [resolved:\"YYYY-MM-DD\"] \
                        [loan:true/false] [address:eth_address]".to_string());
        }

        let mut proposal_name = None;
        let mut new_title = None;
        let mut url = None;
        let mut team = None;
        let mut amounts = None;
        let mut start_date = None;
        let mut end_date = None;
        let mut announced_date = None;
        let mut published_date = None;
        let mut resolved_date = None;
        let mut is_loan = None;
        let mut payment_address = None;

        for arg in args {
            if let Some((key, value)) = arg.split_once(':') {
                match key {
                    "proposal" => proposal_name = Some(value.to_string()),
                    "title" => new_title = Some(value.to_string()),
                    "url" => url = Some(value.to_string()),
                    "team" => team = Some(value.to_string()),
                    "amounts" => amounts = Some(Self::parse_amounts(value)?),
                    "start" => start_date = Some(value.to_string()),
                    "end" => end_date = Some(value.to_string()),
                    "announced" => announced_date = Some(value.to_string()),
                    "published" => published_date = Some(value.to_string()),
                    "resolved" => resolved_date = Some(value.to_string()),
                    "loan" => {
                        is_loan = Some(value.parse::<bool>()
                            .map_err(|_| format!("Invalid loan value: {}", value))?);
                    },
                    "address" => payment_address = Some(value.to_string()),
                    _ => return Err(format!("Unknown parameter: {}", key))
                }
            }
        }

        Ok(UpdateProposalArgs {
            proposal_name: proposal_name.ok_or("Missing proposal name parameter")?,
            new_title,
            url,
            team,
            amounts,
            start_date,
            end_date,
            announced_date,
            published_date,
            resolved_date,
            is_loan,
            payment_address,
        })
    }

    fn parse_close_proposal(args: &[String]) -> Result<CloseProposalArgs, String> {
        let mut name = None;
        let mut resolution = None;

        for arg in args {
            if let Some((key, value)) = arg.split_once(':') {
                match key.to_lowercase().as_str() {
                    "name" => name = Some(value.to_string()),
                    "res" => {
                        // Case-insensitive match for resolution
                        let res = match value.to_lowercase().as_str() {
                            "approved" => "Approved",
                            "rejected" => "Rejected",
                            "invalid" => "Invalid",
                            "duplicate" => "Duplicate",
                            "retracted" => "Retracted",
                            _ => return Err(format!("Invalid resolution: {}. Must be one of: Approved, Rejected, Invalid, Duplicate, Retracted", value)),
                        };
                        resolution = Some(res.to_string());
                    },
                    _ => return Err(format!("Unknown parameter: {}", key)),
                }
            }
        }

        Ok(CloseProposalArgs {
            name: name.ok_or("Missing name parameter")?,
            resolution: resolution.ok_or("Missing resolution parameter")?,
        })
    }

    fn parse_process_vote(args: &[String]) -> Result<ProcessVoteArgs, String> {
        let mut name = None;
        let mut counted_votes = HashMap::new();
        let mut uncounted_votes = HashMap::new();
        let mut vote_opened = None;
        let mut vote_closed = None;

        fn parse_votes(votes_str: &str) -> Result<HashMap<String, VoteChoice>, String> {
            votes_str
                .split(',')
                .map(|vote| {
                    let parts: Vec<&str> = vote.split(':').collect();
                    if parts.len() != 2 {
                        return Err(format!("Invalid vote format: {}. Expected Team:Choice", vote));
                    }
                    let choice = match parts[1].to_lowercase().as_str() {
                        "yes" => VoteChoice::Yes,
                        "no" => VoteChoice::No,
                        _ => return Err(format!("Invalid vote choice: {}. Must be Yes or No", parts[1])),
                    };
                    Ok((parts[0].to_string(), choice))
                })
                .collect()
        }

        for arg in args {
            if let Some((key, value)) = arg.split_once(':') {
                match key.to_lowercase().as_str() {
                    "name" => name = Some(value.to_string()),
                    "counted" => counted_votes = parse_votes(value)?,
                    "uncounted" => uncounted_votes = parse_votes(value)?,
                    "opened" => vote_opened = Some(Self::parse_date(value)?),
                    "closed" => vote_closed = Some(Self::parse_date(value)?),
                    _ => return Err(format!("Unknown parameter: {}", key)),
                }
            }
        }

        Ok(ProcessVoteArgs {
            name: name.ok_or("Missing name parameter")?,
            counted_votes,
            uncounted_votes,
            vote_opened,
            vote_closed,
        })
    }

    fn parse_create_raffle(args: &[String]) -> Result<CreateRaffleArgs, String> {
        let mut proposal_name = None;
        let mut block_offset = None;
        let mut excluded_teams = None;

        for arg in args {
            if let Some((key, value)) = arg.split_once(':') {
                match key.to_lowercase().as_str() {
                    "name" => proposal_name = Some(value.to_string()),
                    "block_offset" => {
                        block_offset = Some(value.parse::<u64>()
                            .map_err(|_| format!("Invalid block offset: {}", value))?)
                    },
                    "excluded" => {
                        excluded_teams = Some(value.split(',')
                            .map(|s| s.trim().to_string())
                            .collect());
                    },
                    _ => return Err(format!("Unknown parameter: {}", key)),
                }
            } else {
                return Err(format!("Invalid argument format: {}. Expected key:value", arg));
            }
        }

        Ok(CreateRaffleArgs {
            proposal_name: proposal_name.ok_or("Missing required parameter: name")?,
            block_offset,
            excluded_teams,
        })
    }
    
}

pub async fn execute_command(
    telegram_cmd: TelegramCommand,
    budget_system: &mut BudgetSystem,
) -> Result<String, String> {
    match telegram_cmd {
        TelegramCommand::Help => {
            Ok(format!("{}", TelegramCommand::descriptions()))
        },

        TelegramCommand::PrintTeamReport => {
            budget_system.execute_command(Command::PrintTeamReport { team_name: None }).await
            .map(|s| escape_markdown(&s))
            .map_err(|e| format!("Command failed: {}", e))
        },

        TelegramCommand::PrintEpochState => {
            budget_system.execute_command(Command::PrintEpochState).await
            .map(|s| escape_markdown(&s))
            .map_err(|e| format!("Command failed: {}", e))
        },

        TelegramCommand::PrintTeamParticipation { team_name, epoch_name } => {
            budget_system.execute_command(Command::PrintTeamVoteParticipation { 
                team_name, 
                epoch_name: Some(epoch_name)
            }).await
            .map(|s| escape_markdown(&s))
            .map_err(|e| format!("Command failed: {}", e))
        },

        TelegramCommand::CreateEpoch { name, start_date, end_date } => {
            let start_date = TelegramCommand::parse_start_date(&start_date)
                .map_err(|e| format!("Invalid start date: {}", e))?;
            let end_date = TelegramCommand::parse_end_date(&end_date)
                .map_err(|e| format!("Invalid end date: {}", e))?;

            budget_system.execute_command(Command::CreateEpoch { 
                name, 
                start_date, 
                end_date
            }).await
            .map(|s| escape_markdown(&s))
            .map_err(|e| format!("Command failed: {}", e))
        },

        TelegramCommand::ActivateEpoch { name } => {
            budget_system.execute_command(Command::ActivateEpoch { name }).await
            .map(|s| escape_markdown(&s))
            .map_err(|e| format!("Command failed: {}", e))
        },

        TelegramCommand::SetEpochReward { token, amount } => {
            let amount = amount.parse::<f64>()
                .map_err(|e| format!("Invalid amount: {}", e))?;
            budget_system.execute_command(Command::SetEpochReward { token, amount }).await
            .map(|s| escape_markdown(&s))
            .map_err(|e| format!("Command failed: {}", e))
        },

        TelegramCommand::AddTeam { args } => {
            let args = TelegramCommand::parse_command(&args)
                .map_err(|e| format!("Failed to parse team arguments: {}", e))?;
            
            let team_args = TelegramCommand::parse_add_team(&args)
                .map_err(|e| format!("Failed to parse team details: {}", e))?;
            
            budget_system.execute_command(Command::AddTeam { 
                name: team_args.name,
                representative: team_args.representative,
                trailing_monthly_revenue: team_args.revenue,
                address: team_args.address,
            }).await
            .map(|s| escape_markdown(&s))
            .map_err(|e| format!("Command failed: {}", e))
        },
        
        TelegramCommand::UpdateTeam { args } => {
            let args = TelegramCommand::parse_command(&args)
                .map_err(|e| format!("Failed to parse team arguments: {}", e))?;
            
            let update_args = TelegramCommand::parse_update_team(&args)
                .map_err(|e| format!("Failed to parse team update details: {}", e))?;
            
            if let Some(status) = &update_args.status {
                match status.to_lowercase().as_str() {
                    "earner" if update_args.revenue.is_none() => {
                        return Err("Revenue data is required for Earner status".into());
                    },
                    "earner" | "supporter" | "inactive" => (), // valid statuses
                    _ => return Err(format!("Invalid status: {}", status).into()),
                }
            }
            
            budget_system.execute_command(Command::UpdateTeam {
                team_name: update_args.team,
                updates: UpdateTeamDetails {
                    name: update_args.new_name,
                    representative: update_args.representative,
                    status: update_args.status,
                    trailing_monthly_revenue: update_args.revenue,
                    address: update_args.address,
                }
            }).await
            .map(|s| escape_markdown(&s))
            .map_err(|e| format!("Command failed: {}", e))
        }

        TelegramCommand::AddProposal { args } => {
            let args = TelegramCommand::parse_command(&args)
                .map_err(|e| format!("Failed to parse proposal arguments: {}", e))?;
            
            let proposal_args = TelegramCommand::parse_add_proposal(&args)
                .map_err(|e| format!("Failed to parse proposal details: {}", e))?;
            
            let budget_request_details = if proposal_args.team.is_some() || proposal_args.amounts.is_some() {
                Some(BudgetRequestDetailsCommand {
                    team: proposal_args.team,
                    request_amounts: proposal_args.amounts,
                    start_date: proposal_args.start_date
                        .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                    end_date: proposal_args.end_date
                        .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                    is_loan: proposal_args.is_loan,
                    payment_address: proposal_args.payment_address
                })
            } else {
                None
            };

            budget_system.execute_command(Command::AddProposal {
                title: proposal_args.title,
                url: Some(proposal_args.url),
                budget_request_details,
                announced_at: proposal_args.announced_date
                    .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                published_at: proposal_args.published_date
                    .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                is_historical: None,
            }).await
            .map(|s| escape_markdown(&s))
            .map_err(|e| format!("Command failed: {}", e))
        },
        
        TelegramCommand::UpdateProposal { args } => {
            let args = TelegramCommand::parse_command(&args)
                .map_err(|e| format!("Failed to parse proposal arguments: {}", e))?;
            
            let update_args = TelegramCommand::parse_update_proposal(&args)
                .map_err(|e| format!("Failed to parse proposal update details: {}", e))?;
            
            let budget_request_details = if update_args.team.is_some() || update_args.amounts.is_some() {
                Some(BudgetRequestDetailsCommand {
                    team: update_args.team,
                    request_amounts: update_args.amounts,
                    start_date: update_args.start_date
                        .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                    end_date: update_args.end_date
                        .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                    is_loan: update_args.is_loan,
                    payment_address: update_args.payment_address,
                })
            } else {
                None
            };

            budget_system.execute_command(Command::UpdateProposal {
                proposal_name: update_args.proposal_name,
                updates: UpdateProposalDetails {
                    title: update_args.new_title,
                    url: update_args.url,
                    budget_request_details,
                    announced_at: update_args.announced_date
                        .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                    published_at: update_args.published_date
                        .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                    resolved_at: update_args.resolved_date
                        .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                }
            }).await
            .map(|s| escape_markdown(&s))
            .map_err(|e| format!("Command failed: {}", e))
        },

        TelegramCommand::CloseProposal { args } => {
            let args = TelegramCommand::parse_command(&args)
                .map_err(|e| format!("Failed to parse arguments: {}", e))?;
            
            let parsed_args = TelegramCommand::parse_close_proposal(&args)
                .map_err(|e| format!("Failed to parse close proposal arguments: {}", e))?;
            
            budget_system.execute_command(Command::CloseProposal { 
                proposal_name: parsed_args.name, 
                resolution: parsed_args.resolution 
            }).await
            .map(|s| escape_markdown(&s))
            .map_err(|e| format!("Command failed: {}", e))
        },

        TelegramCommand::ProcessVote { args } => {
            let args = TelegramCommand::parse_command(&args)
                .map_err(|e| format!("Failed to parse arguments: {}", e))?;
            
            let parsed_args = TelegramCommand::parse_process_vote(&args)
                .map_err(|e| format!("Failed to parse vote arguments: {}", e))?;
            
            budget_system.execute_command(Command::CreateAndProcessVote {
                proposal_name: parsed_args.name,
                counted_votes: parsed_args.counted_votes,
                uncounted_votes: parsed_args.uncounted_votes,
                vote_opened: parsed_args.vote_opened,
                vote_closed: parsed_args.vote_closed,
            }).await
            .map(|s| escape_markdown(&s))
            .map_err(|e| format!("Command failed: {}", e))
        }

        TelegramCommand::CreateRaffle { args } => {
            let args = TelegramCommand::parse_command(&args)
                .map_err(|e| format!("Failed to parse arguments: {}", e))?;
            
            let parsed_args = TelegramCommand::parse_create_raffle(&args)
                .map_err(|e| format!("Failed to parse raffle arguments: {}", e))?;
            
            budget_system.execute_command(Command::CreateRaffle { 
                proposal_name: parsed_args.proposal_name, 
                block_offset: parsed_args.block_offset, 
                excluded_teams: parsed_args.excluded_teams, 
            }).await
            .map(|s| escape_markdown(&s))
            .map_err(|e| format!("Command failed: {}", e))
        },

        TelegramCommand::GenerateUnpaidReport { args } => {
            let epoch_name = if args.is_empty() {
                None
            } else {
                Some(args)
            };
        
            let temp_dir = std::env::temp_dir();
            let output_path = temp_dir.join(format!("unpaid_report_{}.json", Utc::now().timestamp()));
            
            // First generate the report file
            budget_system.generate_unpaid_requests_report(
                Some(output_path.to_str().unwrap()),
                epoch_name.as_deref()
            ).map_err(|e| e.to_string())?;
        
            // Read and format the JSON
            let json_content = std::fs::read_to_string(&output_path)
                .map_err(|e| format!("Failed to read report: {}", e))?;
            
            // Parse and re-serialize to ensure proper formatting
            let json_value: serde_json::Value = serde_json::from_str(&json_content)
                .map_err(|e| format!("Failed to parse JSON: {}", e))?;
            let formatted_json = serde_json::to_string_pretty(&json_value)
                .map_err(|e| format!("Failed to format JSON: {}", e))?;
            
            // Return the formatted message
            Ok(format!("{}\n\n```json\n\n{}\n\n```", 
                escape_markdown("Unpaid Requests Report:"),
                formatted_json
            ))
            
        },

        TelegramCommand::EpochPayments { epoch_name } => {
            budget_system.execute_command(Command::GenerateEpochPaymentsReport { 
                epoch_name, 
                output_path: None 
            }).await
            .map(|s| escape_markdown(&s))
            .map_err(|e| format!("Command failed: {}", e))
        },

        TelegramCommand::LogPayment { args } => {
            let args = TelegramCommand::parse_command(&args)
                .map_err(|e| format!("Failed to parse arguments: {}", e))?;
        
            let mut tx = None;
            let mut date = None;
            let mut proposals = None;
        
            for arg in args {
                if let Some((key, value)) = arg.split_once(':') {
                    match key.to_lowercase().as_str() {
                        "tx" => tx = Some(value.to_string()),
                        "date" => date = Some(NaiveDate::parse_from_str(value, "%Y-%m-%d")
                            .map_err(|e| format!("Invalid date format: {}", e))?),
                        "proposals" => proposals = Some(value.split(',')
                            .map(String::from)
                            .collect::<Vec<String>>()),
                        _ => return Err(format!("Unknown parameter: {}", key)),
                    }
                }
            }
        
            let tx = tx.ok_or("Missing tx parameter")?;
            let date = date.ok_or("Missing date parameter")?;
            let proposals = proposals.ok_or("Missing proposals parameter")?;

            budget_system.execute_command(Command::LogPayment {
                payment_tx: tx,
                payment_date: date,
                proposal_names: proposals 
            }).await
            .map(|s| escape_markdown(&s))
            .map_err(|e| format!("Command failed: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use teloxide::utils::command::BotCommands;
    use chrono::TimeZone;

    use crate::core::budget_system::BudgetSystem;
    use crate::core::models::BudgetRequestDetails;
    use crate::core::models::Resolution;
    use crate::services::ethereum::MockEthereumService;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn create_test_budget_system() -> (BudgetSystem, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = crate::app_config::AppConfig {
            state_file: temp_dir.path().join("test_state.json").to_str().unwrap().to_string(),
            ipc_path: "/tmp/test_reth.ipc".to_string(),
            future_block_offset: 10,
            script_file: "test_script.json".to_string(),
            default_total_counted_seats: 7,
            default_max_earner_seats: 5,
            default_qualified_majority_threshold: 0.7,
            counted_vote_points: 5,
            uncounted_vote_points: 2,
            telegram: crate::app_config::TelegramConfig {
                chat_id: "test_chat_id".to_string(),
                token: "test_token".to_string(),
            },
        };
        let ethereum_service = Arc::new(MockEthereumService::new());
        let budget_system = BudgetSystem::new(config, ethereum_service, None).await.unwrap();
        (budget_system, temp_dir)
    }

    #[test]
    fn test_parse_command_args() {
        let input = "title:Test Proposal url:https://google.com/test?q=1 team:Test Team";
        let args = TelegramCommand::parse_command(input).unwrap();
        assert_eq!(args[0], "title:Test Proposal");
        assert_eq!(args[1], "url:https://google.com/test?q=1");
        assert_eq!(args[2], "team:Test Team");
    }

    #[test]
    fn test_parse_add_team_args() {
        let args = TelegramCommand::parse_command("name:Test Team rep:John Doe rev:1000,2000,3000").unwrap();
        let team_args = TelegramCommand::parse_add_team(&args).unwrap();
        assert_eq!(team_args.name, "Test Team");
        assert_eq!(team_args.representative, "John Doe");
        assert_eq!(team_args.revenue, Some(vec![1000, 2000, 3000]));
    }

    #[test]
    fn test_parse_add_team_args_no_revenue() {
        let args = TelegramCommand::parse_command("name:Support Team rep:Jane Doe").unwrap();
        let team_args = TelegramCommand::parse_add_team(&args).unwrap();
        assert_eq!(team_args.name, "Support Team");
        assert_eq!(team_args.representative, "Jane Doe");
        assert_eq!(team_args.revenue, None);
    }

    #[test]
    fn test_parse_add_proposal_args() {
        let input = "title:Test Proposal url:https://example.com team:Test Team amounts:ETH:100";
        let args = TelegramCommand::parse_command(input).unwrap();
        let proposal_args = TelegramCommand::parse_add_proposal(&args).unwrap();
        assert_eq!(proposal_args.title, "Test Proposal");
        assert_eq!(proposal_args.url, "https://example.com");
        assert_eq!(proposal_args.team, Some("Test Team".to_string()));
        assert_eq!(proposal_args.amounts, Some(HashMap::from([("ETH".to_string(), 100.0)])));
    }

    #[test]
    fn test_parse_add_proposal_args_with_dates() {
        let input = "title:Test url:https://test.com start:2024-01-01 end:2024-12-31";
        let args = TelegramCommand::parse_command(input).unwrap();
        let proposal_args = TelegramCommand::parse_add_proposal(&args).unwrap();
        assert_eq!(proposal_args.title, "Test");
        assert_eq!(proposal_args.start_date, Some("2024-01-01".to_string()));
        assert_eq!(proposal_args.end_date, Some("2024-12-31".to_string()));
    }

    #[test]
    fn test_invalid_args() {
        assert!(TelegramCommand::parse_command("invalid input").is_err());
        
        let args = TelegramCommand::parse_command("name:Test Team invalid:param").unwrap();
        assert!(TelegramCommand::parse_add_team(&args).is_err());
    }
    

    #[test]
    fn test_parse_help_command() {
        assert!(matches!(
            TelegramCommand::parse("/help", "bot_name").unwrap(),
            TelegramCommand::Help
        ));
    }

    #[test]
    fn test_parse_print_team_report_command() {
        assert!(matches!(
            TelegramCommand::parse("/print_team_report", "bot_name").unwrap(),
            TelegramCommand::PrintTeamReport
        ));
    }

    #[test]
    fn test_parse_print_team_participation_command() {
        let cmd = TelegramCommand::parse("/print_team_participation TeamA EpochB", "bot_name").unwrap();
        match cmd {
            TelegramCommand::PrintTeamParticipation { team_name, epoch_name } => {
                assert_eq!(team_name, "TeamA");
                assert_eq!(epoch_name, "EpochB");
            },
            _ => panic!("Wrong command parsed")
        }
    }

    #[test]
    fn test_parse_create_epoch_command() {
        let cmd = TelegramCommand::parse(
            "/create_epoch TestEpoch 2024-01-01 2024-12-31", 
            "bot_name"
        ).unwrap();
        match cmd {
            TelegramCommand::CreateEpoch { name, start_date, end_date } => {
                assert_eq!(name, "TestEpoch");
                assert_eq!(start_date, "2024-01-01");
                assert_eq!(end_date, "2024-12-31");
            },
            _ => panic!("Wrong command parsed")
        }
    }

    #[test]
    fn test_parse_activate_epoch_command() {
        let cmd = TelegramCommand::parse("/activate_epoch TestEpoch", "bot_name").unwrap();
        match cmd {
            TelegramCommand::ActivateEpoch { name } => {
                assert_eq!(name, "TestEpoch");
            },
            _ => panic!("Wrong command parsed")
        }
    }

    #[test]
    fn test_parse_set_epoch_reward_command() {
        let cmd = TelegramCommand::parse("/set_epoch_reward ETH 100.5", "bot_name").unwrap();
        match cmd {
            TelegramCommand::SetEpochReward { token, amount } => {
                assert_eq!(token, "ETH");
                assert_eq!(amount, "100.5");
            },
            _ => panic!("Wrong command parsed")
        }
    }

    // Add tests for error cases
    #[test]
    fn test_parse_print_team_participation_missing_args() {
        assert!(TelegramCommand::parse("/print_team_participation", "bot_name").is_err());
        assert!(TelegramCommand::parse("/print_team_participation TeamA", "bot_name").is_err());
    }

    #[test]
    fn test_parse_create_epoch_invalid_args() {
        assert!(TelegramCommand::parse("/create_epoch", "bot_name").is_err());
        assert!(TelegramCommand::parse("/create_epoch TestEpoch", "bot_name").is_err());
        assert!(TelegramCommand::parse("/create_epoch TestEpoch 2024-01-01", "bot_name").is_err());
    }
    
    #[test]
    fn test_activate_epoch_command() {
        let cmd = TelegramCommand::parse("/activate_epoch TestEpoch", "bot_name").unwrap();
        if let TelegramCommand::ActivateEpoch { name } = cmd {
            assert_eq!(name, "TestEpoch");
        } else {
            panic!("Wrong command parsed");
        }
    }

    #[test]
    fn test_set_epoch_reward_command() {
        let cmd = TelegramCommand::parse("/set_epoch_reward ETH 100.5", "bot_name").unwrap();
        if let TelegramCommand::SetEpochReward { token, amount } = cmd {
            assert_eq!(token, "ETH");
            assert_eq!(amount, "100.5");
        } else {
            panic!("Wrong command parsed");
        }
    }

    #[test]
    fn test_invalid_commands() {
        assert!(TelegramCommand::parse("/unknown_command", "bot_name").is_err());
        assert!(TelegramCommand::parse("/create_epoch", "bot_name").is_err()); // Missing arguments
        assert!(TelegramCommand::parse("/set_epoch_reward ETH", "bot_name").is_err()); // Missing amount
    }

    #[test]
    fn test_date_parsing_edge_cases() {
        // Test leap year
        assert!(TelegramCommand::parse_date("2024-02-29").is_ok());
        assert!(TelegramCommand::parse_date("2023-02-29").is_err());
        
        // Test invalid days
        assert!(TelegramCommand::parse_date("2024-04-31").is_err());
        assert!(TelegramCommand::parse_date("2024-06-31").is_err());
        
        // Test boundary dates
        assert!(TelegramCommand::parse_date("9999-12-31").is_ok());
        assert!(TelegramCommand::parse_date("0000-01-01").is_ok());
    }

    #[test]
    fn test_date_parsing() {
        // Test start date parsing (00:00:00 UTC)
        let start_result = TelegramCommand::parse_start_date("2024-01-01").unwrap();
        assert_eq!(
            start_result,
            Utc.ymd(2024, 1, 1).and_hms_opt(0, 0, 0).unwrap()
        );

        // Test end date parsing (23:59:59 UTC)
        let end_result = TelegramCommand::parse_end_date("2024-01-01").unwrap();
        assert_eq!(
            end_result,
            Utc.ymd(2024, 1, 1).and_hms_opt(23, 59, 59).unwrap()
        );

        // Test invalid dates
        assert!(TelegramCommand::parse_start_date("2024-13-01").is_err()); // Invalid month
        assert!(TelegramCommand::parse_end_date("01/01/2024").is_err()); // Wrong format
    }

    #[test]
    fn test_date_boundaries() {
        let start = TelegramCommand::parse_start_date("2024-01-01").unwrap();
        let end = TelegramCommand::parse_end_date("2024-01-01").unwrap();
        
        assert_eq!(start.time(), chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        assert_eq!(end.time(), chrono::NaiveTime::from_hms_opt(23, 59, 59).unwrap());
        
        // Test day difference
        assert_eq!((end - start).num_seconds(), 86399); // 23:59:59 worth of seconds
    }

    #[test]
    fn test_parse_amounts() {
        let amounts = TelegramCommand::parse_amounts("ETH:124.0,USD:124500").unwrap();
        assert_eq!(amounts.get("ETH").unwrap(), &124.0);
        assert_eq!(amounts.get("USD").unwrap(), &124500.0);
    }

    #[test]
    fn test_parse_amounts_invalid() {
        assert!(TelegramCommand::parse_amounts("ETH:invalid").is_err());
        assert!(TelegramCommand::parse_amounts("invalid_format").is_err());
        assert!(TelegramCommand::parse_amounts("ETH:100:extra").is_err());
    }

    #[test]
    fn test_update_proposal_args() {
        let input = "proposal:Test title:New Title amounts:ETH:100.5,USD:1000";
        let args = TelegramCommand::parse_command(input).unwrap();
        let update_args = TelegramCommand::parse_update_proposal(&args).unwrap();
        
        assert_eq!(update_args.proposal_name, "Test");
        assert_eq!(update_args.new_title, Some("New Title".to_string()));
        
        let amounts = update_args.amounts.unwrap();
        assert_eq!(amounts.get("ETH").unwrap(), &100.5);
        assert_eq!(amounts.get("USD").unwrap(), &1000.0);
    }

    #[test]
    fn test_update_team_args() {
        let input = "team:Old Team name:New Team rep:New Rep status:Supporter rev:1000,2000,3000";
        let args = TelegramCommand::parse_command(input).unwrap();
        let update_args = TelegramCommand::parse_update_team(&args).unwrap();
        
        assert_eq!(update_args.team, "Old Team");
        assert_eq!(update_args.new_name, Some("New Team".to_string()));
        assert_eq!(update_args.representative, Some("New Rep".to_string()));
        assert_eq!(update_args.status, Some("Supporter".to_string()));
        assert_eq!(update_args.revenue, Some(vec![1000, 2000, 3000]));
    }

    #[test]
    fn test_proposal_with_complex_amounts() {
        let input = "title:Test Proposal url:https://test.com amounts:ETH:123.456,USD:100000.50";
        let args = TelegramCommand::parse_command(input).unwrap();
        let proposal_args = TelegramCommand::parse_add_proposal(&args).unwrap();
        
        let amounts = proposal_args.amounts.unwrap();
        assert_eq!(amounts.get("ETH").unwrap(), &123.456);
        assert_eq!(amounts.get("USD").unwrap(), &100000.50);
    }

    #[tokio::test]
    async fn test_add_team_command() {
        let (mut budget_system, _temp_dir) = create_test_budget_system().await;
        
        let command = TelegramCommand::AddTeam {
            args: "name:Test Team rep:John Doe rev:1000,2000,3000".to_string()
        };
        
        let result = execute_command(command, &mut budget_system).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.contains("Added team: Test Team"));
    }

    #[tokio::test]
    async fn test_update_team_command() {
        let (mut budget_system, _temp_dir) = create_test_budget_system().await;
        
        // First add a team
        let add_command = TelegramCommand::AddTeam {
            args: "name:Test Team rep:John Doe rev:1000,2000,3000".to_string()
        };
        let add_result = execute_command(add_command, &mut budget_system).await;
        assert!(add_result.is_ok(), "Failed to add team: {:?}", add_result.err());
        
        // Then update it
        let update_command = TelegramCommand::UpdateTeam {
            args: "team:Test Team name:Updated Team rep:Jane Doe status:Supporter".to_string()
        };
        
        let result = execute_command(update_command, &mut budget_system).await;
        if let Err(e) = &result {
            println!("Error updating team: {}", e);
        }
        assert!(result.is_ok(), "Failed to update team");
        let response = result.unwrap();
        assert!(response.contains("Updated team"));
    }

    #[tokio::test]
    async fn test_add_team_command_invalid_args() {
        let (mut budget_system, _temp_dir) = create_test_budget_system().await;
        
        let command = TelegramCommand::AddTeam {
            args: "invalid args".to_string()
        };
        
        let result = execute_command(command, &mut budget_system).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_team_command_invalid_status() {
        let (mut budget_system, _temp_dir) = create_test_budget_system().await;
        
        // First add a team
        let add_command = TelegramCommand::AddTeam {
            args: "name:Test Team rep:John Doe rev:1000,2000,3000".to_string()
        };
        execute_command(add_command, &mut budget_system).await.unwrap();
        
        // Try to update with invalid status
        let update_command = TelegramCommand::UpdateTeam {
            args: "team:Test Team status:Invalid".to_string()
        };
        
        let result = execute_command(update_command, &mut budget_system).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid status"));
    }

    #[tokio::test]
    async fn test_add_proposal_command() {
        let (mut budget_system, _temp_dir) = create_test_budget_system().await;
        
        // Create epoch first
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(30);
        budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
        budget_system.activate_epoch(budget_system.get_epoch_id_by_name("Test Epoch").unwrap()).unwrap();
        
        let command = TelegramCommand::AddProposal {
            args: "title:Test Proposal url:https://test.com amounts:ETH:100.5,USD:1000".to_string()
        };
        
        let result = execute_command(command, &mut budget_system).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.contains("Added proposal: Test Proposal"));
    }

    #[tokio::test]
    async fn test_update_proposal_command() {
        let (mut budget_system, _temp_dir) = create_test_budget_system().await;
        
        // Setup: Create epoch and proposal
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(30);
        budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
        budget_system.activate_epoch(budget_system.get_epoch_id_by_name("Test Epoch").unwrap()).unwrap();
        
        let add_command = TelegramCommand::AddProposal {
            args: "title:Test Proposal url:https://test.com".to_string()
        };
        execute_command(add_command, &mut budget_system).await.unwrap();
        
        // Update the proposal
        let update_command = TelegramCommand::UpdateProposal {
            args: "proposal:Test Proposal title:Updated Proposal amounts:ETH:200.5".to_string()
        };
        
        let result = execute_command(update_command, &mut budget_system).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.contains("Updated proposal"));
    }

    #[tokio::test]
    async fn test_update_team_command_variants() {
        let (mut budget_system, _temp_dir) = create_test_budget_system().await;
        
        // Add initial team
        let add_command = TelegramCommand::AddTeam {
            args: "name:Test Team rep:John Doe rev:1000,2000,3000".to_string()
        };
        execute_command(add_command, &mut budget_system).await.unwrap();
        
        // Test updating to Supporter status
        let update_to_supporter = TelegramCommand::UpdateTeam {
            args: "team:Test Team status:Supporter".to_string()
        };
        let result = execute_command(update_to_supporter, &mut budget_system).await;
        assert!(result.is_ok(), "Failed to update to Supporter: {:?}", result.err());
        
        // Test updating name only
        let update_name = TelegramCommand::UpdateTeam {
            args: "team:Test Team name:New Name".to_string()
        };
        let result = execute_command(update_name, &mut budget_system).await;
        assert!(result.is_ok(), "Failed to update name: {:?}", result.err());
        
        // Test updating multiple fields
        let update_multiple = TelegramCommand::UpdateTeam {
            args: "team:New Name rep:Jane Doe status:Earner rev:2000,3000,4000".to_string()
        };
        let result = execute_command(update_multiple, &mut budget_system).await;
        assert!(result.is_ok(), "Failed to update multiple fields: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_update_team_command_invalid_cases() {
        let (mut budget_system, _temp_dir) = create_test_budget_system().await;
        
        // Add initial team
        let add_command = TelegramCommand::AddTeam {
            args: "name:Test Team rep:John Doe rev:1000,2000,3000".to_string()
        };
        execute_command(add_command, &mut budget_system).await.unwrap();
        
        // Test invalid status
        let invalid_status = TelegramCommand::UpdateTeam {
            args: "team:Test Team status:Invalid".to_string()
        };
        let result = execute_command(invalid_status, &mut budget_system).await;
        assert!(result.is_err());
        
        // Test Earner without revenue
        let invalid_earner = TelegramCommand::UpdateTeam {
            args: "team:Test Team status:Earner".to_string()
        };
        let result = execute_command(invalid_earner, &mut budget_system).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_close_proposal() {
        let input = "name:Test Proposal res:approved";
        let args = TelegramCommand::parse_command(input).unwrap();
        let result = TelegramCommand::parse_close_proposal(&args).unwrap();
        
        assert_eq!(result.name, "Test Proposal");
        assert_eq!(result.resolution, "Approved");

        // Test case insensitivity
        let input = "name:Test Proposal res:APPROVED";
        let args = TelegramCommand::parse_command(input).unwrap();
        let result = TelegramCommand::parse_close_proposal(&args).unwrap();
        assert_eq!(result.resolution, "Approved");
    }

    #[test]
    fn test_parse_process_vote() {
        let input = "name:Test Proposal counted:TeamA:yes,TeamB:NO uncounted:TeamC:Yes,TeamD:no opened:2024-10-11 closed:2024-10-16";
        let args = TelegramCommand::parse_command(input).unwrap();
        let result = TelegramCommand::parse_process_vote(&args).unwrap();
        
        assert_eq!(result.name, "Test Proposal");
        assert_eq!(result.counted_votes.len(), 2);
        assert_eq!(result.uncounted_votes.len(), 2);
        assert_eq!(result.vote_opened.unwrap(), NaiveDate::from_ymd_opt(2024, 10, 11).unwrap());
        assert_eq!(result.vote_closed.unwrap(), NaiveDate::from_ymd_opt(2024, 10, 16).unwrap());

        // Verify vote choices
        assert_eq!(result.counted_votes.get("TeamA"), Some(&VoteChoice::Yes));
        assert_eq!(result.counted_votes.get("TeamB"), Some(&VoteChoice::No));
    }

    #[test]
    fn test_invalid_resolution() {
        let input = "name:Test Proposal res:invalid_value";
        let args = TelegramCommand::parse_command(input).unwrap();
        let result = TelegramCommand::parse_close_proposal(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_vote_format() {
        let input = "name:Test Proposal counted:TeamA:maybe";
        let args = TelegramCommand::parse_command(input).unwrap();
        let result = TelegramCommand::parse_process_vote(&args);
        assert!(result.is_err());
    }

    // #[test]
    // fn test_parse_create_raffle() {
    //     // Test minimal valid args
    //     let args = vec!["name:Test Proposal".to_string()];
    //     let parsed = TelegramCommand::parse_create_raffle(&args).unwrap();
    //     assert_eq!(parsed.proposal_name, "Test Proposal");
    //     assert!(parsed.block_offset.is_none());
    //     assert!(parsed.excluded_teams.is_none());

    //     // Test full args
    //     let args = vec![
    //         "name:Test Proposal".to_string(),
    //         "block_offset:20".to_string(),
    //         "excluded:Team1,Team2,Team3".to_string(),
    //     ];
    //     let parsed = TelegramCommand::parse_create_raffle(&args).unwrap();
    //     assert_eq!(parsed.proposal_name, "Test Proposal");
    //     assert_eq!(parsed.block_offset, Some(20));
    //     assert_eq!(
    //         parsed.excluded_teams,
    //         Some(vec!["Team1".to_string(), "Team2".to_string(), "Team3".to_string()])
    //     );
    // }

    // #[test]
    // fn test_parse_create_raffle_errors() {
    //     // Test missing name
    //     let args = vec!["block_offset:20".to_string()];
    //     assert!(TelegramCommand::parse_create_raffle(&args).is_err());

    //     // Test invalid block offset
    //     let args = vec![
    //         "name:Test".to_string(),
    //         "block_offset:invalid".to_string(),
    //     ];
    //     assert!(TelegramCommand::parse_create_raffle(&args).is_err());

    //     // Test invalid format
    //     let args = vec!["name=Test".to_string()];
    //     assert!(TelegramCommand::parse_create_raffle(&args).is_err());
    // }

    // // #[tokio::test]
    // // async fn test_create_raffle_command() {
    // //     let (mut budget_system, _temp_dir) = create_test_budget_system().await;
    // //     let start_date = Utc::now();
    // //     let end_date = start_date + chrono::Duration::days(30);
    // //     budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
    // //     budget_system.activate_epoch(budget_system.get_epoch_id_by_name("Test Epoch").unwrap()).unwrap();

    // //     budget_system.add_proposal(
    // //         "Test Proposal".to_string(), 
    // //         None,
    // //         None,
    // //         None,
    // //         None,
    // //         None
    // //     ).unwrap();

    // //     budget_system.create_team("Team1".to_string(), "Rep1".to_string(), Some(vec![1000])).unwrap();
    // //     budget_system.create_team("Team2".to_string(), "Rep2".to_string(), Some(vec![2000])).unwrap();

    // //      // Setup block progression before executing command
    // //     if let Some(mock_service) = get_mock_service(&budget_system) {
    // //         setup_block_progression(mock_service).await;
    // //     }

    // //     let command = TelegramCommand::CreateRaffle {
    // //         args: "name:Test Proposal".to_string()
    // //     };
        
    // //     let result = execute_command(command, &mut budget_system).await;
    // //     assert!(result.is_ok());
        
    // //     let output = result.unwrap();
    // //     // Verify expected message content
    // //     assert!(output.contains("Preparing raffle"));
    // //     assert!(output.contains("ballot range"));
    // //     assert!(output.contains("Completed"));
        
    // //     // Verify escaped markdown
    // //     assert!(output.contains("\\*")); // Should have escaped asterisks
        
    // //     // Verify system state
    // //     assert_eq!(budget_system.state().raffles().len(), 1);
    // // }

    // #[tokio::test]
    // async fn test_create_raffle_command() {
    //     let (mut budget_system, _temp_dir) = create_test_budget_system().await;
    //     let start_date = Utc::now();
    //     let end_date = start_date + chrono::Duration::days(30);
    //     budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
    //     budget_system.activate_epoch(budget_system.get_epoch_id_by_name("Test Epoch").unwrap()).unwrap();
    
    //     budget_system.add_proposal(
    //         "Test Proposal".to_string(),
    //         None,
    //         None,
    //         Some(Utc::now().date_naive()),
    //         Some(Utc::now().date_naive()),
    //         None
    //     ).unwrap();

    //     // Add some teams
    //     budget_system.create_team("Team1".to_string(), "Rep1".to_string(), Some(vec![1000])).unwrap();
    //     budget_system.create_team("Team2".to_string(), "Rep2".to_string(), Some(vec![2000])).unwrap();

    //     // Setup block progression with completion notification
    //     if let Some(mock_service) = get_mock_service(&budget_system) {
    //         let (tx, rx) = tokio::sync::oneshot::channel();
    //         let service = mock_service.clone();
            
    //         tokio::spawn(async move {
    //             for i in 0..5 {
    //                 service.increment_block();
    //                 tokio::time::sleep(Duration::from_millis(50)).await;
    //             }
    //             tx.send(()).unwrap();
    //         });

    //         // Wait for block progression to complete
    //         rx.await.unwrap();
    //     }

    //     let command = TelegramCommand::CreateRaffle {
    //         args: "name:Test Proposal".to_string()
    //     };
        
    //     // Execute command with timeout
    //     let result = execute_command(command, &mut budget_system).await;
        
    //     assert!(result.is_ok(), "Failed to execute command: {:?}", result.err());
    //     let output = result.unwrap();
        
    //     // Verify expected message content
    //     assert!(output.contains("Preparing raffle"));
    //     assert!(output.contains("ballot range"));
    //     assert!(output.contains("Completed"));
        
    //     // Verify escaped markdown
    //     assert!(output.contains("\\*")); // Should have escaped asterisks
        
    //     // Verify system state
    //     assert_eq!(budget_system.state().raffles().len(), 1);
    // }

    // #[tokio::test]
    // async fn test_create_raffle_command_error_cases() {
    //     let (mut budget_system, _temp_dir) = create_test_budget_system().await;
    //     let start_date = Utc::now();
    //     let end_date = start_date + chrono::Duration::days(30);
    //     budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
    //     budget_system.activate_epoch(budget_system.get_epoch_id_by_name("Test Epoch").unwrap()).unwrap();

    //     // Setup block progression before executing command
    //     if let Some(mock_service) = get_mock_service(&budget_system) {
    //         setup_block_progression(mock_service).await;
    //     }
        
    //     // Test invalid proposal
    //     let command = TelegramCommand::CreateRaffle {
    //         args: "name:NonExistent".to_string()
    //     };
        
    //     let result = execute_command(command, &mut budget_system).await;
    //     assert!(result.is_err());
    //     assert!(result.unwrap_err().contains("Failed to prepare raffle"));
        
    //     // Test invalid argument format
    //     let command = TelegramCommand::CreateRaffle {
    //         args: "invalid format".to_string()
    //     };
        
    //     let result = execute_command(command, &mut budget_system).await;
    //     assert!(result.is_err());
    //     assert!(result.unwrap_err().contains("Failed to parse arguments"));
    // }


    #[test]
    fn test_parse_proposal_with_loan_and_address() {
        let input = "title:Test Proposal url:https://test.com loan:true address:0x742d35Cc6634C0532925a3b844Bc454e4438f44e";
        let args = TelegramCommand::parse_command(input).unwrap();
        let proposal_args = TelegramCommand::parse_add_proposal(&args).unwrap();
        
        assert_eq!(proposal_args.title, "Test Proposal");
        assert!(proposal_args.is_loan.unwrap());
        assert_eq!(proposal_args.payment_address.unwrap(), "0x742d35Cc6634C0532925a3b844Bc454e4438f44e");
    }

    #[test]
    fn test_parse_proposal_invalid_loan_value() {
        let input = "title:Test Proposal url:https://test.com loan:invalid";
        let args = TelegramCommand::parse_command(input).unwrap();
        let result = TelegramCommand::parse_add_proposal(&args);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid loan value"));
    }

    #[test]
    fn test_parse_add_proposal_with_loan_and_address() {
        let input = "title:New Proposal url:https://test.com \
                    team:Team1 amounts:ETH:100.5 loan:true \
                    address:0x742d35Cc6634C0532925a3b844Bc454e4438f44e";
        
        let args = TelegramCommand::parse_command(input).unwrap();
        let result = TelegramCommand::parse_add_proposal(&args).unwrap();
        
        assert_eq!(result.title, "New Proposal");
        assert!(result.is_loan.unwrap());
        assert_eq!(result.payment_address.unwrap(), 
            "0x742d35Cc6634C0532925a3b844Bc454e4438f44e");
    }

    #[test]
    fn test_parse_update_proposal_with_loan_and_address() {
        let input = "proposal:Test loan:true \
                    address:0x742d35Cc6634C0532925a3b844Bc454e4438f44e";
        
        let args = TelegramCommand::parse_command(input).unwrap();
        let result = TelegramCommand::parse_update_proposal(&args).unwrap();
        
        assert_eq!(result.proposal_name, "Test");
        assert!(result.is_loan.unwrap());
        assert_eq!(result.payment_address.unwrap(),
            "0x742d35Cc6634C0532925a3b844Bc454e4438f44e");
    }

    #[test]
    fn test_proposal_commands_with_missing_required_fields() {
        // Test add proposal without required fields
        let input = "loan:true";
        let args = TelegramCommand::parse_command(input).unwrap();
        let result = TelegramCommand::parse_add_proposal(&args);
        assert!(result.is_err());

        // Test update proposal without proposal name
        let input = "loan:true address:0x742d35Cc6634C0532925a3b844Bc454e4438f44e";
        let args = TelegramCommand::parse_command(input).unwrap();
        let result = TelegramCommand::parse_update_proposal(&args);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_generate_unpaid_report_command() {
        use crate::core::models::BudgetRequestDetails;
        use crate::core::models::Resolution;

        let (mut budget_system, _temp_dir) = create_test_budget_system().await;
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(30);
        budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
        budget_system.activate_epoch(budget_system.get_epoch_id_by_name("Test Epoch").unwrap()).unwrap();

        let team_id = budget_system.create_team(
            "Test Team".to_string(),
            "Representative".to_string(),
            Some(vec![1000]),
            None
        ).unwrap();

        let mut amounts = HashMap::new();
        amounts.insert("ETH".to_string(), 100.0);
        
        let proposal_id = budget_system.add_proposal(
            "Test Proposal".to_string(),
            None,
            Some(BudgetRequestDetails::new(
                Some(team_id),
                amounts,
                None,
                None,
                Some(false),
                Some("0x742d35Cc6634C0532925a3b844Bc454e4438f44e".to_string()),
            ).unwrap()),
            Some(Utc::now().date_naive()),
            Some(Utc::now().date_naive()),
            None,
        ).unwrap();

        budget_system.close_with_reason(proposal_id, &Resolution::Approved).unwrap();

        // Test command without epoch name
        let command = TelegramCommand::GenerateUnpaidReport { 
            args: "".to_string() 
        };
        
        let result = execute_command(command, &mut budget_system).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert!(response.contains("```json"));
        assert!(response.contains("Test Proposal"));
        assert!(response.contains("Test Team"));

        // Test with epoch name
        let command = TelegramCommand::GenerateUnpaidReport { 
            args: "Test Epoch".to_string() 
        };
        
        let result = execute_command(command, &mut budget_system).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_log_payment_command() {
        let (mut budget_system, _temp_dir) = create_test_budget_system().await;
        
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(30);
        budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
        budget_system.activate_epoch(budget_system.get_epoch_id_by_name("Test Epoch").unwrap()).unwrap();

        let proposal_id = budget_system.add_proposal(
            "Test Proposal".to_string(),
            None,
            Some(BudgetRequestDetails::new(
                None,
                [("ETH".to_string(), 100.0)].iter().cloned().collect(),
                None,
                None,
                Some(false),
                Some("0x742d35Cc6634C0532925a3b844Bc454e4438f44e".to_string()),
            ).unwrap()),
            Some(Utc::now().date_naive()),
            Some(Utc::now().date_naive()),
            None,
        ).unwrap();

        // Approve the proposal
        budget_system.close_with_reason(proposal_id, &Resolution::Approved).unwrap();

        // Test command execution
        let command = TelegramCommand::LogPayment {
            args: "tx:0x742d35Cc6634C0532925a3b844Bc454e4438f44e4438f44e4438f44e4438f44e date:2024-01-01 proposals:Test Proposal".to_string()
        };

        let result = execute_command(command, &mut budget_system).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Payment recorded"));

        // Verify payment was recorded
        let proposal = budget_system.get_proposal(&proposal_id).unwrap();
        assert!(proposal.budget_request_details().unwrap().is_paid());
    }

    #[tokio::test]
    async fn test_log_payment_missing_parameters() {
        let (mut budget_system, _temp_dir) = create_test_budget_system().await;
        
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(30);
        budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
        budget_system.activate_epoch(budget_system.get_epoch_id_by_name("Test Epoch").unwrap()).unwrap();

        let command = TelegramCommand::LogPayment {
            args: "tx:0x742d35Cc6634C0532925a3b844Bc454e4438f44e4438f44e4438f44e4438f44e".to_string()
        };

        let result = execute_command(command, &mut budget_system).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing"));
    }

    #[tokio::test]
    async fn test_log_payment_invalid_date() {
        let (mut budget_system, _temp_dir) = create_test_budget_system().await;
        
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(30);
        budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
        budget_system.activate_epoch(budget_system.get_epoch_id_by_name("Test Epoch").unwrap()).unwrap();

        let command = TelegramCommand::LogPayment {
            args: "tx:0x742d35Cc6634C0532925a3b844Bc454e4438f44e4438f44e4438f44e4438f44e date:invalid proposals:Test Proposal".to_string()
        };

        let result = execute_command(command, &mut budget_system).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid date"));
    }

}
