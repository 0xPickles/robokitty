//src/main.rs

pub mod core {
    pub mod models;
}

mod services;
use services::ethereum::{EthereumService, EthereumServiceTrait};
use services::telegram::{TelegramBot, spawn_command_executor};
use crate::core::models::{
    Team,
    TeamStatus,
    Epoch,
    EpochStatus,
    EpochReward, 
    TeamReward,
    Proposal,
    ProposalStatus,
    Resolution,
    PaymentStatus,
    BudgetRequestDetails,
    NameMatches,
    get_id_by_name,
    Raffle,
    RaffleConfig,
    RaffleResult,
    RaffleTicket,
    Vote,
    VoteType,
    VoteStatus,
    VoteChoice,
    VoteCount,
    VoteParticipation,
    VoteResult,
};


use chrono::{DateTime, NaiveDate, Utc, TimeZone};
use dotenvy::dotenv;
use log::{info, debug, error};
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
    str,
    sync::Arc,
};
use teloxide::prelude::*;
use tokio::{
    self,
    sync::mpsc,
    time::Duration,
};
use uuid::Uuid;

mod app_config;
use app_config::AppConfig;

#[derive(Clone, Serialize, Deserialize)]
struct SystemState {
    teams: HashMap<Uuid, Team>,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
struct BudgetSystemState {
    current_state: SystemState,
    history: Vec<SystemState>,
    proposals: HashMap<Uuid, Proposal>,
    raffles: HashMap<Uuid, Raffle>,
    votes: HashMap<Uuid, Vote>,
    epochs: HashMap<Uuid, Epoch>,
    current_epoch: Option<Uuid>,
}

struct BudgetSystem {
    state: BudgetSystemState,
    ethereum_service: Arc<dyn EthereumServiceTrait>,
    config: AppConfig,
}

impl BudgetSystem {
    async fn new(config: AppConfig, ethereum_service: Arc<dyn EthereumServiceTrait>) -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(parent) = Path::new(&config.state_file).parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        Ok(Self {
            state: BudgetSystemState {
                current_state: SystemState {
                    teams: HashMap::new(),
                    timestamp: Utc::now(),
                },
                history: Vec::new(),
                proposals: HashMap::new(),
                raffles: HashMap::new(),
                votes: HashMap::new(),
                epochs: HashMap::new(),
                current_epoch: None,
            },
            ethereum_service,
            config
        })
    }

    fn add_team(&mut self, name: String, representative: String, trailing_monthly_revenue: Option<Vec<u64>>) -> Result<Uuid, &'static str> {
        let team = Team::new(name, representative, trailing_monthly_revenue)?;
        let id = team.id();
        self.state.current_state.teams.insert(id, team);
        self.save_state();
        Ok(id)
    }

    fn remove_team(&mut self, team_id: Uuid) -> Result<(), &'static str> {
        if self.state.current_state.teams.remove(&team_id).is_some() {
            self.save_state();
            Ok(())
        } else {
            Err("Team not found")
        }
    }

    fn update_team_status(&mut self, team_id: Uuid, new_status: &TeamStatus) -> Result<(), &'static str> {
        match self.state.current_state.teams.get_mut(&team_id) {
            Some(team) => {
                team.set_status(new_status.clone())?;
                self.save_state();
                Ok(())
            },
            None => Err("Team not found"),
        }
    }

    fn save_state(&self) -> Result<(), Box<dyn std::error::Error>> {
        let state_file = &self.config.state_file;
        info!("Attempting to save state to file: {}", state_file);

        // Ensure the directory for the state file exists
        if let Some(parent) = Path::new(state_file).parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&self.state)?;
        
        // Write to a temporary file first
        let temp_file = format!("{}.temp", state_file);
        fs::write(&temp_file, &json).map_err(|e| {
            error!("Failed to write to temporary file {}: {}", temp_file, e);
            e
        })?;

        // Rename the temporary file to the actual state file
        fs::rename(&temp_file, state_file).map_err(|e| {
            error!("Failed to rename temporary file to {}: {}", state_file, e);
            e
        })?;

        // Verify that the file was actually written
        let written_contents = fs::read_to_string(state_file).map_err(|e| {
            error!("Failed to read back the state file {}: {}", state_file, e);
            e
        })?;

        if written_contents != json {
            error!("State file contents do not match what was supposed to be written!");
            return Err("State file verification failed".into());
        }

        info!("Successfully saved and verified state to file: {}", state_file);
        Ok(())
    }

    fn load_state(path: &str) -> Result<BudgetSystemState, Box<dyn std::error::Error>> {
        let json = fs::read_to_string(path)?;
        let state: BudgetSystemState = serde_json::from_str(&json)?;
        Ok(state)
    }

    pub async fn load_from_file(
        path: &str,
        config: AppConfig,
        ethereum_service: Arc<dyn EthereumServiceTrait>
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Ensure the directory for the state file exists
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        let state = if Path::new(path).exists() {
            Self::load_state(path)?
        } else {
            BudgetSystemState {
                current_state: SystemState {
                    teams: HashMap::new(),
                    timestamp: Utc::now(),
                },
                history: Vec::new(),
                proposals: HashMap::new(),
                raffles: HashMap::new(),
                votes: HashMap::new(),
                epochs: HashMap::new(),
                current_epoch: None,
            }
        };
        
        Ok(Self {
            state,
            ethereum_service,
            config,
        })
    }

    fn add_proposal(&mut self, title: String, url: Option<String>, budget_request_details: Option<BudgetRequestDetails>, announced_at: Option<NaiveDate>, published_at: Option<NaiveDate>, is_historical: Option<bool>) -> Result<Uuid, &'static str> {
        let current_epoch_id = self.state.current_epoch.ok_or("No active epoch")?;
    
        let proposal = Proposal::new(current_epoch_id, title, url, budget_request_details, announced_at, published_at, is_historical);
        let proposal_id = proposal.id();
        self.state.proposals.insert(proposal_id, proposal);

        if let Some(epoch) = self.state.epochs.get_mut(&current_epoch_id) {
            epoch.add_proposal(proposal_id);
        } else {
            return Err("Current epoch not found");
        }
        self.save_state();
        Ok(proposal_id)
    }

    fn close_with_reason(&mut self, id: Uuid, resolution: &Resolution) -> Result<(), &'static str> {
        if let Some(proposal) = self.state.proposals.get_mut(&id) {
            if proposal.is_closed() {
                return Err("Proposal is already closed");
            }
            if let Some(details) = &proposal.budget_request_details() {
                if details.is_paid() {
                    return Err("Cannot close: Proposal is already paid");
                }
            }
            proposal.set_resolution(Some(resolution.clone()));
            proposal.set_status(ProposalStatus::Closed);
            self.save_state();
            Ok(())
        } else {
            Err("Proposal not found")
        }
    }

    fn create_formal_vote(&mut self, proposal_id: Uuid, raffle_id: Uuid, threshold: Option<f64>) -> Result<Uuid, &'static str> {
        let proposal = self.state.proposals.get(&proposal_id)
            .ok_or("Proposal not found")?;

        if !proposal.is_actionable() {
            return Err("Proposal is not in a votable state");
        }

        let epoch_id = proposal.epoch_id();

        let raffle = &self.state.raffles.get(&raffle_id)
            .ok_or("Raffle not found")?;

        if raffle.result().is_none() {
            return Err("Raffle results have not been generated");
        }

        let config = raffle.config();

        let vote_type = VoteType::Formal { 
            raffle_id,
            total_eligible_seats: config.total_counted_seats() as u32,
            threshold: self.config.default_qualified_majority_threshold,
            counted_points: self.config.counted_vote_points,
            uncounted_points: self.config.uncounted_vote_points
        };

        let vote = Vote::new(proposal_id, epoch_id, vote_type, false);


        let vote_id = vote.id();
        self.state.votes.insert(vote_id, vote);
        self.save_state();
        Ok(vote_id)
    }

    fn create_informal_vote(&mut self, proposal_id: Uuid) -> Result<Uuid, &'static str> {
        let proposal = self.state.proposals.get(&proposal_id)
            .ok_or("Proposal not found")?;

        if !proposal.is_actionable() {
            return Err("Proposal is not in a votable state");
        }

        let epoch_id = proposal.epoch_id();

        let vote = Vote::new(proposal_id, epoch_id, VoteType::Informal, false);

        let vote_id = vote.id();
        self.state.votes.insert(vote_id, vote);
        self.save_state();
        Ok(vote_id)
    }

    fn cast_votes(&mut self, vote_id: Uuid, votes: Vec<(Uuid, VoteChoice)>) -> Result<(), &'static str> {
        let vote = self.state.votes.get_mut(&vote_id).ok_or("Vote not found")?;

        let raffle_result = match &vote.vote_type() {
            VoteType::Formal { raffle_id, .. } => {
                self.state.raffles.get(raffle_id)
                    .and_then(|raffle| raffle.result())
            },
            VoteType::Informal => None,
        };

        for (team_id, choice) in votes {
            vote.cast_vote(team_id, choice, raffle_result)?;
        }

        self.save_state();
        Ok(())
    }

    fn close_vote(&mut self, vote_id: Uuid) -> Result<bool, &'static str> {
        let vote = self.state.votes.get_mut(&vote_id).ok_or("Vote not found")?;
        
        if vote.is_closed() {
            return Err("Vote is already closed");
        }

        vote.close()?;

        let result = match vote.result() {
            Some(VoteResult::Formal { passed, .. }) => *passed,
            Some(VoteResult::Informal { .. }) => false,
            None => return Err("Vote result not available"),
        };

        self.save_state();
        Ok(result)
    }

    fn create_epoch(&mut self, name: &str, start_date:DateTime<Utc>, end_date: DateTime<Utc>) -> Result<Uuid, &'static str> {
        let new_epoch = Epoch::new(name.to_string(), start_date, end_date)?;

        // Check for overlapping epochs
        for epoch in self.state.epochs.values() {
            if (start_date < epoch.end_date() && end_date > epoch.start_date()) ||
            (epoch.start_date() < end_date && epoch.end_date() > start_date) {
                return Err("New epoch overlaps with an existing epoch");
            }
        }

        let epoch_id = new_epoch.id();
        self.state.epochs.insert(epoch_id, new_epoch);
        self.save_state();
        Ok(epoch_id)
    }

    fn activate_epoch(&mut self, epoch_id: Uuid) -> Result<(), &'static str> {
        if self.state.current_epoch.is_some() {
            return Err("Another epoch is currently active");
        }

        let epoch = self.state.epochs.get_mut(&epoch_id).ok_or("Epoch not found")?;

        epoch.activate();
        
        self.state.current_epoch = Some(epoch_id);

        self.save_state();
        Ok(())
    }

    fn set_epoch_reward(&mut self, token: &str, amount: f64) -> Result<(), &'static str> {
        let epoch_id = self.state.current_epoch.ok_or("No active epoch")?;
        let epoch = self.state.epochs.get_mut(&epoch_id).ok_or("Epoch not found")?;
        
        epoch.set_reward(token.to_string(), amount);
        self.save_state();
        Ok(())
    }

    fn get_current_epoch(&self) -> Option<&Epoch> {
        self.state.current_epoch.and_then(|id| self.state.epochs.get(&id))
    }

    fn get_proposals_for_epoch(&self, epoch_id: Uuid) -> Vec<&Proposal> {
        if let Some(epoch) = self.state.epochs.get(&epoch_id) {
            epoch.associated_proposals().iter()
                .filter_map(|&id| self.state.proposals.get(&id))
                .collect()
        } else {
            vec![]
        }
    }

    fn update_epoch_dates(&mut self, epoch_id: Uuid, new_start: DateTime<Utc>, new_end: DateTime<Utc>) -> Result<(), &'static str> {
        // Check for overlaps with other epochs
        for other_epoch in self.state.epochs.values() {
            if other_epoch.id() != epoch_id &&
               ((new_start < other_epoch.end_date() && new_end > other_epoch.start_date()) ||
                (other_epoch.start_date() < new_end && other_epoch.end_date() > new_start)) {
                return Err("New dates overlap with an existing epoch");
            }
        }
        
        let epoch = self.state.epochs.get_mut(&epoch_id).ok_or("Epoch not found")?;

        if !epoch.is_planned() {
            return Err("Can only modify dates of planned epochs");
        }

        epoch.set_dates(new_start, new_end);

        Ok(())
    }

    pub fn get_team_id_by_name(&self, name: &str) -> Option<Uuid> {
        get_id_by_name(&self.state.current_state.teams, name)
    }

    pub fn get_epoch_id_by_name(&self, name: &str) -> Option<Uuid> {
        get_id_by_name(&self.state.epochs, name)
    }

    pub fn get_proposal_id_by_name(&self, name: &str) -> Option<Uuid> {
        get_id_by_name(&self.state.proposals, name)
    }

    pub fn get_raffle_id_by_name(&self, name: &str) -> Option<Uuid> {
        get_id_by_name(&self.state.raffles, name)
    }

    fn import_predefined_raffle(
        &mut self,
        proposal_name: &str,
        counted_teams: Vec<String>,
        uncounted_teams: Vec<String>,
        total_counted_seats: usize,
        max_earner_seats: usize
    ) -> Result<Uuid, Box<dyn Error>> {
        let proposal_id = self.get_proposal_id_by_name(proposal_name)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_name))?;
        
        let epoch_id = self.state.current_epoch
            .ok_or("No active epoch")?;

        let counted_team_ids: Vec<Uuid> = counted_teams.iter()
            .filter_map(|name| self.get_team_id_by_name(name))
            .collect();
        
        let uncounted_team_ids: Vec<Uuid> = uncounted_teams.iter()
            .filter_map(|name| self.get_team_id_by_name(name))
            .collect();

        // Check if total_counted_seats matches the number of counted teams
        if counted_team_ids.len() != total_counted_seats {
            return Err(format!(
                "Mismatch between specified total_counted_seats ({}) and actual number of counted teams ({})",
                total_counted_seats, counted_team_ids.len()
            ).into());
        }

        // Additional check to ensure max_earner_seats is not greater than total_counted_seats
        if max_earner_seats > total_counted_seats {
            return Err(format!(
                "max_earner_seats ({}) cannot be greater than total_counted_seats ({})",
                max_earner_seats, total_counted_seats
            ).into());
        }

        let raffle_config = RaffleConfig::new(
            proposal_id,
            epoch_id,
            total_counted_seats,
            max_earner_seats,
            Some(0),
            Some(0),
            Some("N/A".to_string()),
            Some(Vec::new()),
            None,
            Some(counted_team_ids.iter().chain(uncounted_team_ids.iter()).cloned().collect()),
            true,
        );

        let mut raffle = Raffle::new(raffle_config, &self.state.current_state.teams)?;
        raffle.set_result(RaffleResult::new(counted_team_ids, uncounted_team_ids));

        let raffle_id = raffle.id();
        self.state.raffles.insert(raffle_id, raffle);
        self.save_state()?;

        Ok(raffle_id)
    }

    fn import_historical_vote(
        &mut self,
        proposal_name: &str,
        passed: bool,
        participating_teams: Vec<String>,
        non_participating_teams: Vec<String>,
        counted_points: Option<u32>,
        uncounted_points: Option<u32>
    ) -> Result<Uuid, Box<dyn Error>> {
        let proposal_id = self.get_proposal_id_by_name(proposal_name)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_name))?;
    
        let raffle_id = self.get_raffle_id_by_name(proposal_name)
            .ok_or_else(|| format!("No raffle found for proposal: {}", proposal_name))?;
    
        let raffle = self.state.raffles.get(&raffle_id)
            .ok_or_else(|| format!("Raffle not found: {}", raffle_id))?;
    
        let epoch_id = raffle.config().epoch_id();
    
        let vote_type = VoteType::Formal {
            raffle_id,
            total_eligible_seats: raffle.config().total_counted_seats() as u32,
            threshold: self.config.default_qualified_majority_threshold,
            counted_points: counted_points.unwrap_or(self.config.counted_vote_points),
            uncounted_points: uncounted_points.unwrap_or(self.config.uncounted_vote_points)
        };
    
        let mut vote = Vote::new(proposal_id, epoch_id, vote_type, true);
    
        // Determine participation
        let (participating_ids, _) = self.determine_participation(
            raffle,
            &participating_teams,
            &non_participating_teams
        )?;
    
        let raffle_result = raffle.result().ok_or("Raffle result not found")?;
    
        // Set participation without casting actual votes
        for &team_id in &participating_ids {
            if raffle_result.counted().contains(&team_id) {
                vote.add_participant(team_id, true)?;
            } else if raffle_result.uncounted().contains(&team_id) {
                vote.add_participant(team_id, false)?;
            }
        }
    
        // Close the vote
        vote.close()?;
    
        // Set the result manually for historical votes
        let result = VoteResult::Formal {
            counted: VoteCount::new(),  // All zeros
            uncounted: VoteCount::new(),  // All zeros
            passed,
        };
        vote.set_result(Some(result));
    
        // Set dates (using current time as a placeholder)
        let now = Utc::now();
        vote.set_opened_at(now);
        vote.set_closed_at(Some(now));
    
        let vote_id = vote.id();
        self.state.votes.insert(vote_id, vote);
    
        // Update proposal status based on vote result
        let proposal = self.state.proposals.get_mut(&proposal_id)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_id))?;
        
        if passed {
            proposal.approve()?;
        } else {
            proposal.reject()?;
        }
        proposal.set_status(ProposalStatus::Closed);
    
        self.save_state()?;
    
        Ok(vote_id)
    }

    fn determine_participation(
        &self,
        raffle: &Raffle,
        participating_teams: &[String],
        non_participating_teams: &[String]
    ) -> Result<(Vec<Uuid>, Vec<Uuid>), Box<dyn Error>> {
        let raffle_result = raffle.result()
            .ok_or("Raffle result not found")?;

        let all_team_ids: Vec<Uuid> = raffle_result.counted().iter()
            .chain(raffle_result.uncounted().iter())
            .cloned()
            .collect();

        if !participating_teams.is_empty() {
            let participating_ids: Vec<Uuid> = participating_teams.iter()
                .filter_map(|name| self.get_team_id_by_name(name))
                .collect();
            let non_participating_ids: Vec<Uuid> = all_team_ids.into_iter()
                .filter(|id| !participating_ids.contains(id))
                .collect();
            Ok((participating_ids, non_participating_ids))
        } else if !non_participating_teams.is_empty() {
            let non_participating_ids: Vec<Uuid> = non_participating_teams.iter()
                .filter_map(|name| self.get_team_id_by_name(name))
                .collect();
            let participating_ids: Vec<Uuid> = all_team_ids.into_iter()
                .filter(|id| !non_participating_ids.contains(id))
                .collect();
            Ok((participating_ids, non_participating_ids))
        } else {
            Ok((all_team_ids, Vec::new()))
        }
    }

    fn print_team_report(&self) -> String {
        let mut teams: Vec<&Team> = self.state.current_state.teams.values().collect();
        teams.sort_by(|a, b| a.name().cmp(&b.name()));

        let mut report = String::from("Team Report:\n\n");

        for team in teams {
            report.push_str(&format!("Name: {}\n", team.name()));
            report.push_str(&format!("ID: {}\n", team.id()));
            report.push_str(&format!("Representative: {}\n", team.representative()));
            report.push_str(&format!("Status: {:?}\n", team.status()));

            if let TeamStatus::Earner { trailing_monthly_revenue } = &team.status() {
                report.push_str(&format!("Trailing Monthly Revenue: {:?}\n", trailing_monthly_revenue));
            }

            // Add a breakdown of points per epoch
            report.push_str("Points per Epoch:\n");
            for epoch in self.state.epochs.values() {
                let epoch_points = self.get_team_points_for_epoch(team.id(), epoch.id()).unwrap_or(0);
                report.push_str(&format!("  {}: {} points\n", epoch.name(), epoch_points));
            }

            report.push_str("\n");
        }

        report
    }

    fn print_epoch_state(&self) -> Result<String, Box<dyn Error>> {
        let epoch = self.get_current_epoch().ok_or("No active epoch")?;
        let proposals = self.get_proposals_for_epoch(epoch.id());

        let mut report = String::new();

        // Epoch overview
        report.push_str(&format!("*State of Epoch {}*\n\n", escape_markdown(&epoch.name())));
        report.push_str("🌍 *Overview*\n");
        report.push_str(&format!("ID: `{}`\n", epoch.id()));
        report.push_str(&format!("Start Date: `{}`\n", epoch.start_date().format("%Y-%m-%d %H:%M:%S UTC")));
        report.push_str(&format!("End Date: `{}`\n", epoch.end_date().format("%Y-%m-%d %H:%M:%S UTC")));
        report.push_str(&format!("Status: `{:?}`\n", epoch.status()));

        if let Some(reward) = epoch.reward() {
            report.push_str(&format!("Epoch Reward: `{} {}`\n", reward.amount(), escape_markdown(reward.token())));
        } else {
            report.push_str("Epoch Reward: `Not set`\n");
        }

        report.push_str("\n");

        // Proposal counts
        let mut open_proposals = Vec::new();
        let mut approved_count = 0;
        let mut rejected_count = 0;
        let mut retracted_count = 0;

        for proposal in &proposals {
            match proposal.resolution() {
                Some(Resolution::Approved) => approved_count += 1,
                Some(Resolution::Rejected) => rejected_count += 1,
                Some(Resolution::Retracted) => retracted_count += 1,
                _ => {
                    if proposal.is_actionable() {
                        open_proposals.push(proposal);
                    }
                }
            }
        }

        report.push_str("📊 *Proposals*\n");
        report.push_str(&format!("Total: `{}`\n", proposals.len()));
        report.push_str(&format!("Open: `{}`\n", open_proposals.len()));
        report.push_str(&format!("Approved: `{}`\n", approved_count));
        report.push_str(&format!("Rejected: `{}`\n", rejected_count));
        report.push_str(&format!("Retracted: `{}`\n", retracted_count));

        report.push_str("\n");

        // Open proposals
        if !open_proposals.is_empty() {
            report.push_str("📬 *Open proposals*\n\n");
        
            for proposal in open_proposals {
                report.push_str(&format!("*{}*\n", escape_markdown(proposal.title())));
                if let Some(url) = proposal.url() {
                    report.push_str(&format!("🔗 {}\n", escape_markdown(url)));
                }
                if let Some(details) = proposal.budget_request_details() {
                    if let (Some(start), Some(end)) = (details.start_date(), details.end_date()) {
                        report.push_str(&format!("📆 {} \\- {}\n", 
                            escape_markdown(&start.format("%b %d").to_string()),
                            escape_markdown(&end.format("%b %d").to_string())
                        ));
                    }
                    if !details.request_amounts().is_empty() {
                        let amounts: Vec<String> = details.request_amounts().iter()
                            .map(|(token, amount)| format!("{} {}", 
                                escape_markdown(&amount.to_string()), 
                                escape_markdown(token)
                            ))
                            .collect();
                        report.push_str(&format!("💰 {}\n", amounts.join(", ")));
                    }
                }
                let days_open = self.days_open(proposal);
                report.push_str(&format!("⏳ _{} days open_\n\n", escape_markdown(&days_open.to_string())));
            }
        }

        Ok(report)
    }

    fn print_team_vote_participation(&self, team_name: &str, epoch_name: Option<&str>) -> Result<String, Box<dyn Error>> {
        let team_id = self.get_team_id_by_name(team_name)
            .ok_or_else(|| format!("Team not found: {}", team_name))?;
    
        let epoch = if let Some(name) = epoch_name {
            self.state.epochs.values()
                .find(|e| e.name() == name)
                .ok_or_else(|| format!("Epoch not found: {}", name))?
        } else {
            self.get_current_epoch()
                .ok_or("No active epoch and no epoch specified")?
        };
    
        let mut report = format!("Vote Participation Report for Team: {}\n", team_name);
        report.push_str(&format!("Epoch: {} ({})\n\n", epoch.name(), epoch.id()));
        let mut vote_reports = Vec::new();
        let mut total_points = 0;
    
        for vote_id in epoch.associated_proposals().iter()
            .filter_map(|proposal_id| self.state.votes.values()
                .find(|v| v.proposal_id() == *proposal_id)
                .map(|v| v.id())) 
        {
            let vote = &self.state.votes[&vote_id];
            let (participation_status, points) = match (vote.vote_type(), vote.participation()) {
                (VoteType::Formal { counted_points, uncounted_points, .. }, VoteParticipation::Formal { counted, uncounted }) => {
                    if counted.contains(&team_id) {
                        (Some("Counted"), *counted_points)
                    } else if uncounted.contains(&team_id) {
                        (Some("Uncounted"), *uncounted_points)
                    } else {
                        (None, 0)
                    }
                },
                (VoteType::Informal, VoteParticipation::Informal(participants)) => {
                    if participants.contains(&team_id) {
                        (Some("N/A (Informal)"), 0)
                    } else {
                        (None, 0)
                    }
                },
                _ => (None, 0),
            };
    
            if let Some(status) = participation_status {
                let proposal = self.state.proposals.get(&vote.proposal_id())
                    .ok_or_else(|| format!("Proposal not found for vote: {}", vote_id))?;
    
                let vote_type = match vote.vote_type() {
                    VoteType::Formal { .. } => "Formal",
                    VoteType::Informal => "Informal",
                };
    
                let result = match vote.result() {
                    Some(VoteResult::Formal { passed, .. }) => if *passed { "Passed" } else { "Failed" },
                    Some(VoteResult::Informal { .. }) => "N/A (Informal)",
                    None => "Pending",
                };
    
                total_points += points;
    
                vote_reports.push((
                    vote.opened_at(),
                    format!(
                        "Vote ID: {}\n\
                        Proposal: {}\n\
                        Type: {}\n\
                        Participation: {}\n\
                        Result: {}\n\
                        Points Earned: {}\n\n",
                        vote_id, proposal.title(), vote_type, status, result, points
                    )
                ));
            }
        }
    
        // Sort vote reports by date, most recent first
        vote_reports.sort_by(|a, b| b.0.cmp(&a.0));
    
        // Add total points to the report
        report.push_str(&format!("Total Points Earned: {}\n\n", total_points));
    
        // Add individual vote reports
        for (_, vote_report) in &vote_reports {
            report.push_str(vote_report);
        }
    
        if vote_reports.is_empty() {
            report.push_str("This team has not participated in any votes during this epoch.\n");
        }
    
        Ok(report)
    }

    fn days_open(&self, proposal: &Proposal) -> i64 {
        let announced_date = proposal.announced_at()
            .unwrap_or_else(|| Utc::now().date_naive());
        Utc::now().date_naive().signed_duration_since(announced_date).num_days()
    }

    fn prepare_raffle(&mut self, proposal_name: &str, excluded_teams: Option<Vec<String>>, app_config: &AppConfig) -> Result<(Uuid, Vec<RaffleTicket>), Box<dyn Error>> {
        let proposal_id = self.get_proposal_id_by_name(proposal_name)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_name))?;
        let epoch_id = self.state.current_epoch
            .ok_or("No active epoch")?;

        let excluded_team_ids = excluded_teams.map(|names| {
            names.into_iter()
                .filter_map(|name| self.get_team_id_by_name(&name))
                .collect::<Vec<Uuid>>()
        }).unwrap_or_else(Vec::new);

        let raffle_config = RaffleConfig::new(
            proposal_id,
            epoch_id,
            app_config.default_total_counted_seats,
            app_config.default_max_earner_seats,
            Some(0),
            Some(0),
            Some(String::new()),
            Some(excluded_team_ids),
            None,
            None,
            false
        );

        let raffle = Raffle::new(raffle_config, &self.state.current_state.teams)?;
        let raffle_id = raffle.id();
        let tickets = raffle.tickets().to_vec();
        
        self.state.raffles.insert(raffle_id, raffle);
        self.save_state()?;

        Ok((raffle_id, tickets))
    }

    async fn import_historical_raffle(
        &mut self,
        proposal_name: &str,
        initiation_block: u64,
        randomness_block: u64,
        team_order: Option<Vec<String>>,
        excluded_teams: Option<Vec<String>>,
        total_counted_seats: Option<usize>,
        max_earner_seats: Option<usize>
    ) -> Result<(Uuid, Raffle), Box<dyn Error>> {
        let proposal_id = self.get_proposal_id_by_name(proposal_name)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_name))?;
    
        let epoch_id = self.state.current_epoch
            .ok_or("No active epoch")?;
    
        let randomness = self.ethereum_service.get_randomness(randomness_block).await?;
    
        let custom_team_order = team_order.map(|order| {
            order.into_iter()
                .filter_map(|name| self.get_team_id_by_name(&name))
                .collect::<Vec<Uuid>>()
        });
    
        let excluded_team_ids = excluded_teams.map(|names| {
            names.into_iter()
                .filter_map(|name| self.get_team_id_by_name(&name))
                .collect::<Vec<Uuid>>()
        }).unwrap_or_else(Vec::new);
    
        let total_counted_seats = total_counted_seats.unwrap_or(self.config.default_total_counted_seats);
        let max_earner_seats = max_earner_seats.unwrap_or(self.config.default_max_earner_seats);
    
        if max_earner_seats > total_counted_seats {
            return Err("max_earner_seats cannot be greater than total_counted_seats".into());
        }

        let raffle_config = RaffleConfig::new(
            proposal_id,
            epoch_id,
            total_counted_seats,
            max_earner_seats,
            Some(initiation_block),
            Some(randomness_block),
            Some(randomness),
            Some(excluded_team_ids),
            None,
            custom_team_order,
            true
        );
    
        let mut raffle = Raffle::new(raffle_config, &self.state.current_state.teams)?;
        raffle.generate_ticket_scores()?;
        raffle.select_deciding_teams();
    
        let raffle_id = raffle.id();
        self.state.raffles.insert(raffle_id, raffle.clone());
        self.save_state()?;
    
        Ok((raffle_id, raffle))
    }

    async fn finalize_raffle(&mut self, raffle_id: Uuid, initiation_block: u64, randomness_block: u64, randomness: String) -> Result<Raffle, Box<dyn Error>> {
        let raffle = self.state.raffles.get_mut(&raffle_id)
            .ok_or_else(|| format!("Raffle not found: {}", raffle_id))?;
    
        raffle.config_mut().set_initiation_block(initiation_block);
        raffle.config_mut().set_randomness_block(randomness_block);
        raffle.config_mut().set_block_randomness(randomness);
    
        raffle.generate_ticket_scores()?;
        raffle.select_deciding_teams();
    
        let raffle_clone = raffle.clone();
        self.save_state()?;
    
        Ok(raffle_clone)
    }

    fn group_tickets_by_team(&self, tickets: &[RaffleTicket]) -> Vec<(String, u64, u64)> {
        let mut grouped_tickets: Vec<(String, u64, u64)> = Vec::new();
        let mut current_team: Option<(String, u64, u64)> = None;

        for ticket in tickets {
            let team_name = self.state.current_state.teams.get(&ticket.team_id())
                .map(|team| team.name().to_string())
                .unwrap_or_else(|| format!("Unknown Team ({})", ticket.team_id()));

            match &mut current_team {
                Some((name, _, end)) if *name == team_name => {
                    *end = ticket.index();
                }
                _ => {
                    if let Some(team) = current_team.take() {
                        grouped_tickets.push(team);
                    }
                    current_team = Some((team_name, ticket.index(), ticket.index()));
                }
            }
        }

        if let Some(team) = current_team {
            grouped_tickets.push(team);
        }

        grouped_tickets
    }

    fn create_and_process_vote(
        &mut self,
        proposal_name: &str,
        counted_votes: HashMap<String, VoteChoice>,
        uncounted_votes: HashMap<String, VoteChoice>,
        vote_opened: Option<NaiveDate>,
        vote_closed: Option<NaiveDate>,
    ) -> Result<String, Box<dyn Error>> {
        // Find proposal and raffle
        let (proposal_id, raffle_id) = self.find_proposal_and_raffle(proposal_name)
            .map_err(|e| format!("Failed to find proposal or raffle: {}", e))?;
        
        // Check if the proposal already has a resolution
        let proposal = self.state.proposals.get(&proposal_id)
            .ok_or_else(|| "Proposal not found after ID lookup".to_string())?;
        if proposal.resolution().is_some() {
            return Err("Cannot create vote: Proposal already has a resolution".into());
        }

        // Validate votes
        self.validate_votes(raffle_id, &counted_votes, &uncounted_votes)
            .map_err(|e| format!("Vote validation failed: {}", e))?;
    
        // Create vote
        let vote_id = self.create_formal_vote(proposal_id, raffle_id, None)
            .map_err(|e| format!("Failed to create formal vote: {}", e))?;
    
        // Cast votes
        let all_votes: Vec<(Uuid, VoteChoice)> = counted_votes.into_iter()
            .chain(uncounted_votes)
            .filter_map(|(team_name, choice)| {
                self.get_team_id_by_name(&team_name).map(|id| (id, choice))
            })
            .collect();
        self.cast_votes(vote_id, all_votes)
            .map_err(|e| format!("Failed to cast votes: {}", e))?;
    
        // Update vote dates
        self.update_vote_dates(vote_id, vote_opened, vote_closed)
            .map_err(|e| format!("Failed to update vote dates: {}", e))?;
    
        // Close vote and update proposal
        let passed = self.close_vote_and_update_proposal(vote_id, proposal_id, vote_closed)
            .map_err(|e| format!("Failed to close vote or update proposal: {}", e))?;

        // Generate report
        self.generate_vote_report(vote_id)
    }
    
    fn find_proposal_and_raffle(&self, proposal_name: &str) -> Result<(Uuid, Uuid), Box<dyn Error>> {
        let proposal_id = self.get_proposal_id_by_name(proposal_name)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_name))?;
        
        let raffle_id = self.get_raffle_id_by_name(proposal_name)
            .ok_or_else(|| format!("No raffle found for proposal: {}", proposal_name))?;
    
        Ok((proposal_id, raffle_id))
    }
    
    fn validate_votes(
        &self,
        raffle_id: Uuid,
        counted_votes: &HashMap<String, VoteChoice>,
        uncounted_votes: &HashMap<String, VoteChoice>,
    ) -> Result<(), Box<dyn Error>> {
        let raffle = self.state.raffles.get(&raffle_id)
            .ok_or_else(|| format!("Raffle not found: {}", raffle_id))?;
    
        if !raffle.is_completed() {
            return Err("Raffle has not been conducted yet".into());
        }
    
        self.validate_votes_against_raffle(raffle, counted_votes, uncounted_votes)
    }
    
    fn update_vote_dates(
        &mut self,
        vote_id: Uuid,
        vote_opened: Option<NaiveDate>,
        vote_closed: Option<NaiveDate>,
    ) -> Result<(), Box<dyn Error>> {
        let vote = self.state.votes.get_mut(&vote_id).ok_or("Vote not found")?;
        
        if let Some(opened) = vote_opened {
            let opened_datetime = opened.and_hms_opt(0, 0, 0)
                .map(|naive| Utc.from_utc_datetime(&naive))
                .ok_or("Invalid opened date")?;
            vote.set_opened_at(opened_datetime);
        }
        
        if let Some(closed) = vote_closed {
            let closed_datetime = closed.and_hms_opt(23, 59, 59)
                .map(|naive| Utc.from_utc_datetime(&naive))
                .ok_or("Invalid closed date")?;
            vote.set_closed_at(Some(closed_datetime));
        }
        
        Ok(())
    }
    
    fn close_vote_and_update_proposal(
        &mut self,
        vote_id: Uuid,
        proposal_id: Uuid,
        vote_closed: Option<NaiveDate>,
    ) -> Result<bool, Box<dyn Error>> {
        let passed = self.close_vote(vote_id)?;
        
        let proposal = self.state.proposals.get_mut(&proposal_id)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_id))?;
        
        println!("Proposal status before update: {:?}", proposal.status());
        println!("Proposal resolution before update: {:?}", proposal.resolution());
        
        let result = if passed {
            proposal.approve()
        } else {
            proposal.reject()
        };
    
        match result {
            Ok(()) => {
                if let Some(closed) = vote_closed {
                    proposal.set_resolved_at(Some(closed));
                }
                println!("Proposal status after update: {:?}", proposal.status());
                println!("Proposal resolution after update: {:?}", proposal.resolution());
                self.save_state()?;
                Ok(passed)
            },
            Err(e) => {
                println!("Error updating proposal: {}", e);
                println!("Current proposal state: {:?}", proposal);
                Err(format!("Failed to update proposal: {}", e).into())
            }
        }
    }

    fn generate_vote_report(&self, vote_id: Uuid) -> Result<String, Box<dyn Error>> {
        let vote = self.state.votes.get(&vote_id).ok_or("Vote not found")?;
        let proposal = self.state.proposals.get(&vote.proposal_id()).ok_or("Proposal not found")?;
        let raffle = self.state.raffles.values()
            .find(|r| r.config().proposal_id() == vote.proposal_id())
            .ok_or("Associated raffle not found")?;
    
        let (counted, uncounted) = vote.vote_counts().ok_or("Vote counts not available")?;
        let counted_yes = counted.yes();
        let counted_no = counted.no();
        let total_counted_votes = counted_yes + counted_no;
        
        let total_eligible_seats = match vote.vote_type() {
            VoteType::Formal { total_eligible_seats, .. } => total_eligible_seats,
            _ => &0,
        };
    
        // Calculate absent votes for counted seats only
        let absent = total_eligible_seats.saturating_sub(total_counted_votes as u32);

        let status = match vote.result() {
            Some(VoteResult::Formal { passed, .. }) => if *passed { "Approved" } else { "Not Approved" },
            Some(VoteResult::Informal { .. }) => "N/A (Informal)",
            None => "Pending",
        };
    
        let deciding_teams: Vec<String> = raffle.deciding_teams().iter()
            .filter_map(|&team_id| {
                self.state.current_state.teams.get(&team_id).map(|team| team.name().to_string())
            })
            .collect();
    
        // Calculate uncounted votes
        let total_uncounted_votes = uncounted.yes() + uncounted.no();
        let total_uncounted_seats = raffle.result()
            .map(|result| result.uncounted().len())
            .unwrap_or(0) as u32;

        let (counted_votes_info, uncounted_votes_info) = if let VoteParticipation::Formal { counted, uncounted } = &vote.participation() {
            let absent_counted: Vec<String> = raffle.result().expect("Raffle result not found").counted().iter()
                .filter(|&team_id| !counted.contains(team_id))
                .filter_map(|&team_id| self.state.current_state.teams.get(&team_id).map(|team| team.name().to_string()))
                .collect();

            let absent_uncounted: Vec<String> = raffle.result().expect("Raffle result not found").uncounted().iter()
                .filter(|&team_id| !uncounted.contains(team_id))
                .filter_map(|&team_id| self.state.current_state.teams.get(&team_id).map(|team| team.name().to_string()))
                .collect();

            let counted_info = if absent_counted.is_empty() {
                format!("Counted votes cast: {}/{}", total_counted_votes, total_eligible_seats)
            } else {
                format!("Counted votes cast: {}/{} ({} absent)", total_counted_votes, total_eligible_seats, absent_counted.join(", "))
            };

            let uncounted_info = if absent_uncounted.is_empty() {
                format!("Uncounted votes cast: {}/{}", total_uncounted_votes, total_uncounted_seats)
            } else {
                format!("Uncounted votes cast: {}/{} ({} absent)", total_uncounted_votes, total_uncounted_seats, absent_uncounted.join(", "))
            };

            (counted_info, uncounted_info)
        } else {
            (
                format!("Counted votes cast: {}/{}", total_counted_votes, total_eligible_seats),
                format!("Uncounted votes cast: {}/{}", total_uncounted_votes, total_uncounted_seats)
            )
        };
    
    
        let report = format!(
            "**{}**\n{}\n\n**Status: {}**\n__{} in favor, {} against, {} absent__\n\n**Deciding teams**\n`{:?}`\n\n{}\n{}",
            proposal.title(),
            proposal.url().as_deref().unwrap_or(""),
            status,
            counted_yes,
            counted_no,
            absent,
            deciding_teams,
            counted_votes_info,
            uncounted_votes_info
        );
    
        Ok(report)
    }

    fn validate_votes_against_raffle(
        &self,
        raffle: &Raffle,
        counted_votes: &HashMap<String, VoteChoice>,
        uncounted_votes: &HashMap<String, VoteChoice>,
    ) -> Result<(), Box<dyn Error>> {
        let raffle_result = raffle.result().ok_or("Raffle result not found")?;
    
        let counted_team_ids: HashSet<_> = raffle_result.counted().iter().cloned().collect();
        let uncounted_team_ids: HashSet<_> = raffle_result.uncounted().iter().cloned().collect();
    
        for team_name in counted_votes.keys() {
            let team_id = self.get_team_id_by_name(team_name)
                .ok_or_else(|| format!("Team not found: {}", team_name))?;
            if !counted_team_ids.contains(&team_id) {
                return Err(format!("Team {} is not eligible for counted vote", team_name).into());
            }
        }
    
        for team_name in uncounted_votes.keys() {
            let team_id = self.get_team_id_by_name(team_name)
                .ok_or_else(|| format!("Team not found: {}", team_name))?;
            if !uncounted_team_ids.contains(&team_id) {
                return Err(format!("Team {} is not eligible for uncounted vote", team_name).into());
            }
        }
    
        Ok(())
    }

    fn update_proposal(&mut self, proposal_name: &str, updates: UpdateProposalDetails) -> Result<(), &'static str> {
        // Find the team_id if it's needed
        let team_id = if let Some(budget_details) = &updates.budget_request_details {
            if let Some(team_name) = &budget_details.team {
                self.get_team_id_by_name(team_name)
            } else {
                None
            }
        } else {
            None
        };
    
        // Update the proposal
        let proposal = self.state.proposals.values_mut()
            .find(|p| p.title() == proposal_name)
            .ok_or("Proposal not found")?;
    
        proposal.update(updates, team_id)?;
    
        self.save_state();
        Ok(())
    }

    fn generate_markdown_test(&self) -> String {
        let test_message = r#"
*Bold text*
_Italic text_
__Underline__
~Strikethrough~
*Bold _italic bold ~italic bold strikethrough~ __underline italic bold___ bold*
[inline URL](http://www.example.com/)
[inline mention of a user](tg://user?id=123456789)
`inline fixed-width code`
```python
def hello_world():
    print("Hello, World!")
```
"#;
        test_message.to_string()
    }

    fn generate_proposal_report(&self, proposal_id: Uuid) -> Result<String, Box<dyn Error>> {
        debug!("Generating proposal report for ID: {:?}", proposal_id);
    
        let proposal = self.state.proposals.get(&proposal_id)
            .ok_or_else(|| format!("Proposal not found: {:?}", proposal_id))?;
    
        debug!("Found proposal: {:?}", proposal.title());
    
        let mut report = String::new();
    
        // Main title (moved outside of Summary)
        report.push_str(&format!("# Proposal Report: {}\n\n", proposal.title()));
    
        // Summary
        report.push_str("## Summary\n\n");
        if let (Some(announced), Some(resolved)) = (proposal.announced_at(), proposal.resolved_at()) {
            let resolution_days = self.calculate_days_between(announced, resolved);
            report.push_str(&format!("This proposal was resolved in {} days from its announcement date. ", resolution_days));
        }
    
        if let Some(vote) = self.state.votes.values().find(|v| v.proposal_id() == proposal_id) {
            if let Some(result) = vote.result() {
                match result {
                    VoteResult::Formal { counted, uncounted, passed } => {
                        report.push_str(&format!("The proposal was {} with {} votes in favor and {} votes against. ", 
                            if *passed { "approved" } else { "not approved" }, 
                            counted.yes(), counted.yes() + uncounted.yes()));
                    },
                    VoteResult::Informal { count } => {
                        report.push_str(&format!("This was an informal vote with {} votes in favor and {} votes against. ", 
                            count.yes(), count.no()));
                    }
                }
            }
        } else {
            report.push_str("No voting information is available for this proposal. ");
        }
    
        if let Some(budget_details) = proposal.budget_request_details() {
            report.push_str(&format!("The budget request was for {} {} for the period from {} to {}. ",
                budget_details.request_amounts().values().sum::<f64>(),
                budget_details.request_amounts().keys().next().unwrap_or(&String::new()),
                budget_details.start_date().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string()),
                budget_details.end_date().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string())
            ));
        }
    
        report.push_str("\n\n");
    
        // Proposal Details
        report.push_str("## Proposal Details\n\n");
        report.push_str(&format!("- **ID**: {}\n", proposal.id()));
        report.push_str(&format!("- **Title**: {}\n", proposal.title()));
        report.push_str(&format!("- **URL**: {}\n", proposal.url().as_deref().unwrap_or("N/A")));
        report.push_str(&format!("- **Status**: {:?}\n", proposal.status()));
        report.push_str(&format!("- **Resolution**: {}\n", proposal.resolution().as_ref().map_or("N/A".to_string(), |r| format!("{:?}", r))));
        report.push_str(&format!("- **Announced**: {}\n", proposal.announced_at().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string())));
        report.push_str(&format!("- **Published**: {}\n", proposal.published_at().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string())));
        report.push_str(&format!("- **Resolved**: {}\n", proposal.resolved_at().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string())));
        report.push_str(&format!("- **Is Historical**: {}\n\n", proposal.is_historical()));
    
        // Budget Request Details
        if let Some(budget_details) = proposal.budget_request_details() {
            report.push_str("## Budget Request Details\n\n");
            report.push_str(&format!("- **Requesting Team**: {}\n", 
                budget_details.team()
                    .and_then(|id| self.state.current_state.teams.get(&id))
                    .map_or("N/A".to_string(), |team| team.name().to_string())));
            report.push_str("- **Requested Amount(s)**:\n");
            for (token, amount) in budget_details.request_amounts() {
                report.push_str(&format!("  - {}: {}\n", token, amount));
            }
            report.push_str(&format!("- **Start Date**: {}\n", budget_details.start_date().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string())));
            report.push_str(&format!("- **End Date**: {}\n", budget_details.end_date().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string())));
            report.push_str(&format!("- **Payment Status**: {:?}\n\n", budget_details.payment_status()));
        }
    
        // Raffle Information
        if let Some(raffle) = self.state.raffles.values().find(|r| r.config().proposal_id() == proposal_id) {
            report.push_str("## Raffle Information\n\n");
            report.push_str(&format!("- **Raffle ID**: {}\n", raffle.id()));
            report.push_str(&format!("- **Initiation Block**: {}\n", raffle.config().initiation_block()));
            report.push_str(&format!("- **Randomness Block**: [{}]({})\n", 
                raffle.config().randomness_block(), raffle.etherscan_url()));
            report.push_str(&format!("- **Block Randomness**: {}\n", raffle.config().block_randomness()));
            report.push_str(&format!("- **Total Counted Seats**: {}\n", raffle.config().total_counted_seats()));
            report.push_str(&format!("- **Max Earner Seats**: {}\n", raffle.config().max_earner_seats()));
            report.push_str(&format!("- **Is Historical**: {}\n\n", raffle.config().is_historical()));
    
            // Team Snapshots
            report.push_str(&self.generate_team_snapshots_table(raffle));
    
            // Raffle Outcome
            if let Some(result) = raffle.result() {
                report.push_str("### Raffle Outcome\n\n");
                self.generate_raffle_outcome(&mut report, raffle, result);
            }
        } else {
            report.push_str("## Raffle Information\n\nNo raffle was conducted for this proposal.\n\n");
        }
    
        // Voting Information
        if let Some(vote) = self.state.votes.values().find(|v| v.proposal_id() == proposal_id) {
            report.push_str("## Voting Information\n\n");
            report.push_str("### Vote Details\n\n");
            report.push_str(&format!("- **Vote ID**: {}\n", vote.id()));
            report.push_str(&format!("- **Type**: {:?}\n", vote.vote_type()));
            report.push_str(&format!("- **Status**: {:?}\n", vote.status()));
            report.push_str(&format!("- **Opened**: {}\n", vote.opened_at().format("%Y-%m-%d %H:%M:%S")));
            if let Some(closed_at) = vote.closed_at() {
                report.push_str(&format!("- **Closed**: {}\n", closed_at.format("%Y-%m-%d %H:%M:%S")));
            }
            if let Some(result) = vote.result() {
                match result {
                    VoteResult::Formal { passed, .. } => {
                        report.push_str(&format!("- **Result**: {}\n\n", if *passed { "Passed" } else { "Not Passed" }));
                    },
                    VoteResult::Informal { .. } => {
                        report.push_str("- **Result**: Informal (No Pass/Fail)\n\n");
                    }
                }
            }
    
            // Participation
            report.push_str("### Participation\n\n");
            report.push_str(&self.generate_vote_participation_tables(vote));
    
            // Vote Counts
            if !vote.is_historical() {
                report.push_str("### Vote Counts\n");
                match vote.vote_type() {
                    VoteType::Formal { total_eligible_seats, .. } => {
                        if let Some(VoteResult::Formal { counted, uncounted, .. }) = vote.result() {
                            let absent = *total_eligible_seats as i32 - (counted.yes() + counted.no()) as i32;
                            
                            report.push_str("#### Counted Votes\n");
                            report.push_str(&format!("- **Yes**: {}\n", counted.yes()));
                            report.push_str(&format!("- **No**: {}\n", counted.no()));
                            if absent > 0 {
                                report.push_str(&format!("- **Absent**: {}\n", absent));
                            }
    
                            report.push_str("\n#### Uncounted Votes\n");
                            report.push_str(&format!("- **Yes**: {}\n", uncounted.yes()));
                            report.push_str(&format!("- **No**: {}\n", uncounted.no()));
                        }
                    },
                    VoteType::Informal => {
                        if let Some(VoteResult::Informal { count }) = vote.result() {
                            report.push_str(&format!("- **Yes**: {}\n", count.yes()));
                            report.push_str(&format!("- **No**: {}\n", count.no()));
                        }
                    }
                }
            } else {
                report.push_str("Vote counts not available for historical votes.\n");
            }
        } else {
            report.push_str("## Voting Information\n\nNo vote was conducted for this proposal.\n\n");
        }
    
        Ok(report)
    }

    fn generate_team_snapshots_table(&self, raffle: &Raffle) -> String {
        let mut table = String::from("### Team Snapshots\n\n");
        table.push_str("| Team Name | Status | Revenue | Ballot Range | Ticket Count |\n");
        table.push_str("|-----------|--------|---------|--------------|--------------|\n");

        for snapshot in raffle.team_snapshots() {
            let team_name = snapshot.name();
            let status = format!("{:?}", snapshot.status());
            let revenue = match snapshot.status() {
                TeamStatus::Earner { trailing_monthly_revenue } => format!("{:?}", trailing_monthly_revenue),
                _ => "N/A".to_string(),
            };
            let tickets: Vec<_> = raffle.tickets().iter()
                .filter(|t| t.team_id() == snapshot.id())
                .collect();
            let ballot_range = if !tickets.is_empty() {
                format!("{} - {}", tickets.first().unwrap().index(), tickets.last().unwrap().index())
            } else {
                "N/A".to_string()
            };
            let ticket_count = tickets.len();

            table.push_str(&format!("| {} | {} | {} | {} | {} |\n", 
                team_name, status, revenue, ballot_range, ticket_count));
        }

        table.push_str("\n");
        table
    }

    fn generate_raffle_outcome(&self, report: &mut String, raffle: &Raffle, result: &RaffleResult) {
        let counted_earners: Vec<_> = result.counted().iter()
            .filter(|&team_id| raffle.team_snapshots().iter().any(|s| s.id() == *team_id && matches!(s.status(), TeamStatus::Earner { .. })))
            .collect();
        let counted_supporters: Vec<_> = result.counted().iter()
            .filter(|&team_id| raffle.team_snapshots().iter().any(|s| s.id() == *team_id && matches!(s.status(), TeamStatus::Supporter)))
            .collect();
    
        report.push_str(&format!("#### Counted Seats (Total: {})\n\n", result.counted().len()));
        
        report.push_str(&format!("##### Earner Seats ({})\n", counted_earners.len()));
        for team_id in counted_earners {
            if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == *team_id) {
                let best_score = raffle.tickets().iter()
                    .filter(|t| t.team_id() == *team_id)
                    .map(|t| t.score())
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or(0.0);
                report.push_str(&format!("- {} (Best Score: {:.4})\n", snapshot.name(), best_score));
            }
        }
    
        report.push_str(&format!("\n##### Supporter Seats ({})\n", counted_supporters.len()));
        for team_id in counted_supporters {
            if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == *team_id) {
                let best_score = raffle.tickets().iter()
                    .filter(|t| t.team_id() == *team_id)
                    .map(|t| t.score())
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or(0.0);
                report.push_str(&format!("- {} (Best Score: {:.4})\n", snapshot.name(), best_score));
            }
        }
    
        report.push_str("\n#### Uncounted Seats\n");
        for team_id in result.uncounted() {
            if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == *team_id) {
                let best_score = raffle.tickets().iter()
                    .filter(|t| t.team_id() == *team_id)
                    .map(|t| t.score())
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or(0.0);
                report.push_str(&format!("- {} (Best Score: {:.4})\n", snapshot.name(), best_score));
            }
        }
    }

    fn generate_vote_participation_tables(&self, vote: &Vote) -> String {
        let mut tables = String::new();

        match &vote.participation() {
            VoteParticipation::Formal { counted, uncounted } => {
                tables.push_str("#### Counted Votes\n");
                tables.push_str("| Team | Points Credited |\n");
                tables.push_str("|------|------------------|\n");
                for &team_id in counted {
                    if let Some(team) = self.state.current_state.teams.get(&team_id) {
                        tables.push_str(&format!("| {} | {} |\n", team.name(), self.config.counted_vote_points));
                    }
                }

                tables.push_str("\n#### Uncounted Votes\n");
                tables.push_str("| Team | Points Credited |\n");
                tables.push_str("|------|------------------|\n");
                for &team_id in uncounted {
                    if let Some(team) = self.state.current_state.teams.get(&team_id) {
                        tables.push_str(&format!("| {} | {} |\n", team.name(), self.config.uncounted_vote_points));
                    }
                }
            },
            VoteParticipation::Informal(participants) => {
                tables.push_str("#### Participants\n");
                tables.push_str("| Team | Points Credited |\n");
                tables.push_str("|------|------------------|\n");
                for &team_id in participants {
                    if let Some(team) = self.state.current_state.teams.get(&team_id) {
                        tables.push_str(&format!("| {} | 0 |\n", team.name()));
                    }
                }
            },
        }

        tables
    }

    fn calculate_days_between(&self, start: NaiveDate, end: NaiveDate) -> i64 {
        (end - start).num_days()
    }

    fn generate_report_file_path(&self, proposal: &Proposal, epoch_name: &str) -> PathBuf {
        debug!("Generating report file path for proposal: {:?}", proposal.id());
    
        let state_file_path = PathBuf::from(&self.config.state_file);
        let state_file_dir = state_file_path.parent().unwrap_or_else(|| {
            debug!("Failed to get parent directory of state file, using current directory");
            Path::new(".")
        });
        let reports_dir = state_file_dir.join("reports").join(epoch_name);
    
        let date = proposal.published_at()
            .or(proposal.announced_at())
            .map(|date| date.format("%Y%m%d").to_string())
            .unwrap_or_else(|| {
                debug!("No published_at or announced_at date for proposal: {:?}", proposal.id());
                "00000000".to_string()
            });
    
        let team_part = proposal.budget_request_details()
            .as_ref()
            .and_then(|details| details.team())
            .and_then(|team_id| self.state.current_state.teams.get(&team_id))
            .map(|team| format!("-{}", clean_file_name(&team.name())))
            .unwrap_or_default();
    
        let truncated_title = clean_file_name(proposal.title())
            .chars()
            .take(30)
            .collect::<String>()
            .replace(" ", "_");
    
        let file_name = format!("{}{}-{}.md", date, team_part, truncated_title);
        debug!("Generated file name: {}", file_name);
    
        reports_dir.join(file_name)
    }

    fn save_report_to_file(&self, content: &str, file_path: &Path) -> Result<(), Box<dyn Error>> {
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(file_path, content)?;
        Ok(())
    }

    fn generate_and_save_proposal_report(&self, proposal_id: Uuid, epoch_name: &str) -> Result<PathBuf, Box<dyn Error>> {
        debug!("Generating report for proposal: {:?}", proposal_id);
    
        let proposal = self.state.proposals.get(&proposal_id)
            .ok_or_else(|| {
                let err = format!("Proposal not found: {:?}", proposal_id);
                error!("{}", err);
                err
            })?;
    
        let report_content = self.generate_proposal_report(proposal_id)?;
        let file_path = self.generate_report_file_path(proposal, epoch_name);
    
        debug!("Saving report to file: {:?}", file_path);
        self.save_report_to_file(&report_content, &file_path)?;
    
        Ok(file_path)
    }

    fn get_current_or_specified_epoch(&self, epoch_name: Option<&str>) -> Result<(&Epoch, Uuid), &'static str> {
        match epoch_name {
            Some(name) => {
                let (id, epoch) = self.state.epochs.iter()
                    .find(|(_, e)| e.name() == name)
                    .ok_or("Specified epoch not found")?;
                Ok((epoch, *id))
            },
            None => {
                let current_epoch_id = self.state.current_epoch.ok_or("No active epoch")?;
                let epoch = self.state.epochs.get(&current_epoch_id).ok_or("Current epoch not found")?;
                Ok((epoch, current_epoch_id))
            }
        }
    }

    fn generate_point_report(&self, epoch_name: Option<&str>) -> Result<String, &'static str> {
        let (epoch, epoch_id) = self.get_current_or_specified_epoch(epoch_name)?;
        self.generate_point_report_for_epoch(epoch_id)
    }

    fn generate_point_report_for_epoch(&self, epoch_id: Uuid) -> Result<String, &'static str> {
        let epoch = self.state.epochs.get(&epoch_id).ok_or("Epoch not found")?;
        let mut report = String::new();

        for (team_id, team) in &self.state.current_state.teams {
            let mut team_report = format!("{}, ", team.name());
            let mut total_points = 0;
            let mut allocations = Vec::new();

            for proposal_id in epoch.associated_proposals() {
                if let Some(proposal) = self.state.proposals.get(&proposal_id) {
                    if let Some(vote) = self.state.votes.values().find(|v| v.proposal_id() == *proposal_id) {
                        let (participation_type, points) = match (vote.vote_type(), vote.participation()) {
                            (VoteType::Formal { counted_points, uncounted_points, .. }, VoteParticipation::Formal { counted, uncounted }) => {
                                if counted.contains(team_id) {
                                    ("Counted", *counted_points)
                                } else if uncounted.contains(team_id) {
                                    ("Uncounted", *uncounted_points)
                                } else {
                                    continue;
                                }
                            },
                            (VoteType::Informal, VoteParticipation::Informal(participants)) => {
                                if participants.contains(team_id) {
                                    ("Informal", 0)
                                } else {
                                    continue;
                                }
                            },
                            _ => continue,
                        };

                        total_points += points;
                        allocations.push(format!("{}: {} voter, {} points", 
                            proposal.title(), participation_type, points));
                    }
                }
            }

            team_report.push_str(&format!("{} points\n", total_points));
            for allocation in allocations {
                team_report.push_str(&format!("{}\n", allocation));
            }
            team_report.push('\n');

            report.push_str(&team_report);
        }

        Ok(report)
    }

    fn get_team_points_history(&self, team_id: Uuid) -> Result<Vec<(Uuid, u32)>, &'static str> {
        self.state.epochs.iter()
            .map(|(&epoch_id, _)| {
                self.get_team_points_for_epoch(team_id, epoch_id)
                    .map(|points| (epoch_id, points))
            })
            .collect()
    }

    fn get_team_points_for_epoch(&self, team_id: Uuid, epoch_id: Uuid) -> Result<u32, &'static str> {
        let epoch = self.state.epochs.get(&epoch_id).ok_or("Epoch not found")?;
        let mut total_points = 0;

        for proposal_id in epoch.associated_proposals() {
            if let Some(vote) = self.state.votes.values().find(|v| v.proposal_id() == *proposal_id) {
                if let (VoteType::Formal { counted_points, uncounted_points, .. }, VoteParticipation::Formal { counted, uncounted }) = (vote.vote_type(), vote.participation()) {
                    if counted.contains(&team_id) {
                        total_points += counted_points;
                    } else if uncounted.contains(&team_id) {
                        total_points += uncounted_points;
                    }
                }
            }
        }

        Ok(total_points)
    }

    fn close_epoch(&mut self, epoch_name: Option<&str>) -> Result<(), Box<dyn Error>> {
        let epoch_id = match epoch_name {
            Some(name) => self.get_epoch_id_by_name(name)
                .ok_or_else(|| format!("Epoch not found: {}", name))?,
            None => self.state.current_epoch
                .ok_or("No active epoch")?
        };
    
        // Collect necessary data
        let actionable_proposals = self.get_proposals_for_epoch(epoch_id)
            .iter()
            .filter(|p| p.is_actionable())
            .count();
    
        if actionable_proposals > 0 {
            return Err(format!("Cannot close epoch: {} actionable proposals remaining", actionable_proposals).into());
        }
    
        let total_points = self.get_total_points_for_epoch(epoch_id);
        let mut team_rewards = HashMap::new();
    
        // Calculate rewards
        if let Some(epoch) = self.state.epochs.get(&epoch_id) {
            if epoch.is_closed() {
                return Err("Epoch is already closed".into());
            }
    
            if let Some(reward) = epoch.reward() {
                if total_points == 0 {
                    return Err("No points earned in this epoch".into());
                }
    
                for (team_id, _) in &self.state.current_state.teams {
                    let team_points = self.calculate_team_points_for_epoch(*team_id, epoch_id);
                    let percentage = team_points as f64 / total_points as f64 * 100.0; // Convert to percentage
                    let amount = reward.amount() * (percentage / 100.0); // Convert percentage back to fraction
    
                    match TeamReward::new(percentage, amount) {
                        Ok(team_reward) => {
                            team_rewards.insert(*team_id, team_reward);
                        },
                        Err(e) => return Err(format!("Failed to create team reward: {}", e).into()),
                    }
                }
            }
        } else {
            return Err("Epoch not found".into());
        }
    
        // Update epoch
        if let Some(epoch) = self.state.epochs.get_mut(&epoch_id) {
            epoch.set_status(EpochStatus::Closed);
            for (team_id, team_reward) in team_rewards {
                epoch.set_team_reward(team_id, team_reward.percentage(), team_reward.amount())?;
            }
        }
    
        // Clear current_epoch if this was the active epoch
        if self.state.current_epoch == Some(epoch_id) {
            self.state.current_epoch = None;
        }
    
        self.save_state()?;
    
        Ok(())
    }

    fn get_total_points_for_epoch(&self, epoch_id: Uuid) -> u32 {
        self.state.current_state.teams.keys()
            .map(|team_id| self.calculate_team_points_for_epoch(*team_id, epoch_id))
            .sum()
    }

    fn calculate_team_points_for_epoch(&self, team_id: Uuid, epoch_id: Uuid) -> u32 {
        let epoch = match self.state.epochs.get(&epoch_id) {
            Some(e) => e,
            None => return 0,
        };

        epoch.associated_proposals().iter()
            .filter_map(|proposal_id| self.state.votes.values().find(|v| v.proposal_id() == *proposal_id))
            .map(|vote| match (vote.vote_type(), vote.participation()) {
                (VoteType::Formal { counted_points, uncounted_points, .. }, VoteParticipation::Formal { counted, uncounted }) => {
                    if counted.contains(&team_id) {
                        *counted_points
                    } else if uncounted.contains(&team_id) {
                        *uncounted_points
                    } else {
                        0
                    }
                },
                _ => 0,
            })
            .sum()
    }

    fn generate_end_of_epoch_report(&self, epoch_name: &str) -> Result<(), Box<dyn Error>> {
        let epoch = self.state.epochs.values()
            .find(|e| e.name() == epoch_name)
            .ok_or_else(|| format!("Epoch not found: {}", epoch_name))?;

        if !epoch.is_closed() {
            return Err("Cannot generate report: Epoch is not closed".into());
        }

        let mut report = String::new();

        // Generate epoch summary
        report.push_str(&self.generate_epoch_summary(epoch)?);

        // Generate proposal tables and individual reports
        report.push_str(&self.generate_proposal_tables(epoch)?);

        // Generate team summary
        report.push_str(&self.generate_team_summary(epoch)?);

        // Save the report
        let file_name = format!("{}-epoch_report.md", Utc::now().format("%Y%m%d"));
        let sanitized_epoch_name = sanitize_filename(epoch_name);
        let report_path = PathBuf::from(&self.config.state_file)
            .parent()
            .unwrap()
            .join("reports")
            .join(sanitized_epoch_name)
            .join(file_name);

        fs::create_dir_all(report_path.parent().unwrap())?;
        fs::write(&report_path, report)?;

        println!("End of Epoch Report generated: {:?}", report_path);

        Ok(())
    }

    fn generate_epoch_summary(&self, epoch: &Epoch) -> Result<String, Box<dyn Error>> {
        let proposals = self.get_proposals_for_epoch(epoch.id());
        let approved = proposals.iter().filter(|p| matches!(p.resolution(), Some(Resolution::Approved))).count();
        let rejected = proposals.iter().filter(|p| matches!(p.resolution(), Some(Resolution::Rejected))).count();
        let retracted = proposals.iter().filter(|p| matches!(p.resolution(), Some(Resolution::Retracted))).count();

        let summary = format!(
            "# End of Epoch Report: {}\n\n\
            ## Epoch Summary\n\
            - **Period**: {} to {}\n\
            - **Total Proposals**: {}\n\
            - **Approved Proposals**: {}\n\
            - **Rejected Proposals**: {}\n\
            - **Retracted Proposals**: {}\n\
            - **Total Reward**: {}\n\n",
            epoch.name(),
            epoch.start_date().format("%Y-%m-%d"),
            epoch.end_date().format("%Y-%m-%d"),
            proposals.len(),
            approved,
            rejected,
            retracted,
            epoch.reward().map_or("N/A".to_string(), |r| format!("{} {}", r.amount(), r.token())),
        );

        Ok(summary)
    }

    fn generate_proposal_tables(&self, epoch: &Epoch) -> Result<String, Box<dyn Error>> {
        let mut tables = String::new();
        let proposals = self.get_proposals_for_epoch(epoch.id());
    
        let statuses = vec![
            ("Approved", Resolution::Approved),
            ("Rejected", Resolution::Rejected),
            ("Retracted", Resolution::Retracted),
        ];
    
        for (status, resolution) in statuses {
            let filtered_proposals: Vec<&Proposal> = proposals.iter()
                .filter(|p| matches!(p.resolution(), Some(r) if r == resolution))
                .map(|p| *p)  // Dereference once to go from &&Proposal to &Proposal
                .collect();
    
            if !filtered_proposals.is_empty() {
                tables.push_str(&format!("### {} Proposals\n", status));
                tables.push_str("| Name | URL | Team | Amounts | Start Date | End Date | Announced | Resolved | Report |\n");
                tables.push_str("|------|-----|------|---------|------------|----------|-----------|----------|---------|\n");
    
                for proposal in &filtered_proposals {
                    // Generate individual proposal report
                    let report_path = self.generate_and_save_proposal_report(proposal.id(), epoch.name())?;
                    let report_link = report_path.file_name().unwrap().to_str().unwrap();
    
                    let team_name = proposal.budget_request_details()
                        .and_then(|d| d.team())
                        .and_then(|id| self.state.current_state.teams.get(&id))
                        .map_or("N/A".to_string(), |t| t.name().to_string());
    
                    let amounts = proposal.budget_request_details()
                        .map(|d| d.request_amounts().iter()
                            .map(|(token, amount)| format!("{} {}", amount, token))
                            .collect::<Vec<_>>()
                            .join(", "))
                        .unwrap_or_else(|| "N/A".to_string());
    
                    tables.push_str(&format!(
                        "| {} | {} | {} | {} | {} | {} | {} | {} | [Report]({}) |\n",
                        proposal.title(),
                        proposal.url().as_deref().unwrap_or("N/A"),
                        team_name,
                        amounts,
                        proposal.budget_request_details().and_then(|d| d.start_date()).map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string()),
                        proposal.budget_request_details().and_then(|d| d.end_date()).map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string()),
                        proposal.announced_at().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string()),
                        proposal.resolved_at().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string()),
                        report_link
                    ));
                }
                tables.push_str("\n");
            }
        }
    
        Ok(tables)
    }
    

    fn generate_team_summary(&self, epoch: &Epoch) -> Result<String, Box<dyn Error>> {
        let mut summary = String::from("## Team Summary\n");
        summary.push_str("| Team Name | Status | Counted Votes | Uncounted Votes | Total Points | % of Total Points | Reward Amount |\n");
        summary.push_str("|-----------|--------|---------------|-----------------|--------------|-------------------|---------------|\n");

        let total_points: u32 = self.state.current_state.teams.keys()
            .map(|team_id| self.get_team_points_for_epoch(*team_id, epoch.id()).unwrap_or(0))
            .sum();

        for (team_id, team) in &self.state.current_state.teams {
            let team_points = self.get_team_points_for_epoch(*team_id, epoch.id()).unwrap_or(0);
            let percentage = if total_points > 0 {
                (team_points as f64 / total_points as f64) * 100.0
            } else {
                0.0
            };

            let (counted_votes, uncounted_votes) = self.get_team_vote_counts(*team_id, epoch.id());

            let reward_amount = epoch.team_rewards().get(team_id)
                .map(|reward| format!("{} {}", reward.amount(), epoch.reward().as_ref().map_or("".to_string(), |r| r.token().to_string())))
                .unwrap_or_else(|| "N/A".to_string());

            summary.push_str(&format!(
                "| {} | {:?} | {} | {} | {} | {:.2}% | {} |\n",
                team.name(),
                team.status(),
                counted_votes,
                uncounted_votes,
                team_points,
                percentage,
                reward_amount
            ));
        }

        Ok(summary)
    }

    fn get_team_vote_counts(&self, team_id: Uuid, epoch_id: Uuid) -> (u32, u32) {
        let mut counted = 0;
        let mut uncounted = 0;

        for vote in self.state.votes.values() {
            if vote.epoch_id() == epoch_id {
                match vote.participation() {
                    VoteParticipation::Formal { counted: c, uncounted: u } => {
                        if c.contains(&team_id) {
                            counted += 1;
                        } else if u.contains(&team_id) {
                            uncounted += 1;
                        }
                    },
                    VoteParticipation::Informal(_) => {}  // Informal votes are not counted here
                }
            }
        }

        (counted, uncounted)
    }
}

// Script commands

#[derive(Deserialize, Clone)]
#[serde(tag = "type", content = "params")]
enum ScriptCommand {
    CreateEpoch { name: String, start_date: DateTime<Utc>, end_date: DateTime<Utc> },
    ActivateEpoch { name: String },
    SetEpochReward { token: String, amount: f64 },
    AddTeam { name: String, representative: String, trailing_monthly_revenue: Option<Vec<u64>> },
    AddProposal {
        title: String,
        url: Option<String>,
        budget_request_details: Option<BudgetRequestDetailsScript>,
        announced_at: Option<NaiveDate>,
        published_at: Option<NaiveDate>,
        is_historical: Option<bool>,
    },
    UpdateProposal {
        proposal_name: String,
        updates: UpdateProposalDetails,
    },
    ImportPredefinedRaffle {
        proposal_name: String,
        counted_teams: Vec<String>,
        uncounted_teams: Vec<String>,
        total_counted_seats: usize,
        max_earner_seats: usize,
    },
    ImportHistoricalVote {
        proposal_name: String,
        passed: bool,
        participating_teams: Vec<String>,
        non_participating_teams: Vec<String>,
        counted_points: Option<u32>,
        uncounted_points: Option<u32>,
    },
    ImportHistoricalRaffle {
        proposal_name: String,
        initiation_block: u64,
        randomness_block: u64,
        team_order: Option<Vec<String>>,
        excluded_teams: Option<Vec<String>>,
        total_counted_seats: Option<usize>,
        max_earner_seats: Option<usize>,
    },
    ChangeTeamStatus {
        team_name: String,
        new_status: String,
        trailing_monthly_revenue: Option<Vec<u64>>,
    },
    PrintTeamReport,
    PrintEpochState,
    PrintTeamVoteParticipation {
        team_name: String,
        epoch_name: Option<String> 
    },
    CloseProposal {
        proposal_name: String,
        resolution: String,
    },
    CreateRaffle {
        proposal_name: String,
        block_offset: Option<u64>,
        excluded_teams: Option<Vec<String>>,
    },
    CreateAndProcessVote {
        proposal_name: String,
        counted_votes: HashMap<String, VoteChoice>,
        uncounted_votes: HashMap<String, VoteChoice>,
        vote_opened: Option<NaiveDate>,
        vote_closed: Option<NaiveDate>,
    },
    GenerateReportsForClosedProposals { epoch_name: String },
    GenerateReportForProposal { proposal_name: String },
    PrintPointReport { epoch_name: Option<String> },
    CloseEpoch { epoch_name: Option<String> },
    GenerateEndOfEpochReport { epoch_name: String },
}

#[derive(Deserialize, Clone)]
pub struct UpdateProposalDetails {
    pub title: Option<String>,
    pub url: Option<String>,
    pub budget_request_details: Option<BudgetRequestDetailsScript>,
    pub announced_at: Option<NaiveDate>,
    pub published_at: Option<NaiveDate>,
    pub resolved_at: Option<NaiveDate>,
}

#[derive(Deserialize, Clone)]
pub struct BudgetRequestDetailsScript {
    pub team: Option<String>,
    pub request_amounts: Option<HashMap<String, f64>>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub payment_status: Option<PaymentStatus>,
}

async fn execute_command(budget_system: &mut BudgetSystem, command: ScriptCommand, config: &AppConfig) -> Result<(), Box<dyn Error>> {
    match command {
        ScriptCommand::CreateEpoch { name, start_date, end_date } => {
            let epoch_id = budget_system.create_epoch(&name, start_date, end_date)?;
            println!("Created epoch: {} ({})", name, epoch_id);
        },
        ScriptCommand::ActivateEpoch { name } => {
            let epoch_id = budget_system.get_epoch_id_by_name(&name)
                .ok_or_else(|| format!("Epoch not found: {}", name))?;
            budget_system.activate_epoch(epoch_id)?;
            println!("Activated epoch: {} ({})", name, epoch_id);
        },
        ScriptCommand::SetEpochReward { token, amount } => {
            budget_system.set_epoch_reward(&token, amount)?;
            println!("Set epoch reward: {} {}", amount, token);
        },
        ScriptCommand::AddTeam { name, representative, trailing_monthly_revenue } => {
            let team_id = budget_system.add_team(name.clone(), representative, trailing_monthly_revenue)?;
            println!("Added team: {} ({})", name, team_id);
        },
        ScriptCommand::AddProposal { title, url, budget_request_details, announced_at, published_at, is_historical } => {
            let budget_request_details = if let Some(details) = budget_request_details {
                let team_id = details.team.as_ref()
                    .and_then(|name| budget_system.get_team_id_by_name(name));
                
                Some(BudgetRequestDetails::new(
                    team_id,
                    details.request_amounts.unwrap_or_default(),
                    details.start_date,
                    details.end_date,
                    details.payment_status
                )?)
            } else {
                None
            };
            
            let proposal_id = budget_system.add_proposal(title.clone(), url, budget_request_details, announced_at, published_at, is_historical)?;
            println!("Added proposal: {} ({})", title, proposal_id);
        },
        ScriptCommand::UpdateProposal { proposal_name, updates } => {
            budget_system.update_proposal(&proposal_name, updates)?;
            println!("Updated proposal: {}", proposal_name);
        },
        ScriptCommand::ImportPredefinedRaffle { 
            proposal_name, 
            counted_teams, 
            uncounted_teams, 
            total_counted_seats, 
            max_earner_seats 
        } => {
            let raffle_id = budget_system.import_predefined_raffle(
                &proposal_name, 
                counted_teams.clone(), 
                uncounted_teams.clone(), 
                total_counted_seats, 
                max_earner_seats
            )?;
            
            let raffle = budget_system.state.raffles.get(&raffle_id).unwrap();

            println!("Imported predefined raffle for proposal '{}' (Raffle ID: {})", proposal_name, raffle_id);
            println!("  Counted teams: {:?}", counted_teams);
            println!("  Uncounted teams: {:?}", uncounted_teams);
            println!("  Total counted seats: {}", total_counted_seats);
            println!("  Max earner seats: {}", max_earner_seats);

            // Print team snapshots
            println!("\nTeam Snapshots:");
            for snapshot in raffle.team_snapshots() {
                println!("  {} ({}): {:?}", snapshot.name(), snapshot.id(), snapshot.status());
            }

            // Print raffle result
            if let Some(result) = raffle.result() {
                println!("\nRaffle Result:");
                println!("  Counted teams: {:?}", result.counted());
                println!("  Uncounted teams: {:?}", result.uncounted());
            } else {
                println!("\nRaffle result not available");
            }
        },
        ScriptCommand::ImportHistoricalVote { 
            proposal_name, 
            passed, 
            participating_teams,
            non_participating_teams,
            counted_points,
            uncounted_points,
        } => {
            let vote_id = budget_system.import_historical_vote(
                &proposal_name,
                passed,
                participating_teams.clone(),
                non_participating_teams.clone(),
                counted_points,
                uncounted_points
            )?;

            let vote = budget_system.state.votes.get(&vote_id).unwrap();
            let proposal = budget_system.state.proposals.get(&vote.proposal_id()).unwrap();

            println!("Imported historical vote for proposal '{}' (Vote ID: {})", proposal_name, vote_id);
            println!("Vote passed: {}", passed);

            println!("\nNon-participating teams:");
            for team_name in &non_participating_teams {
                println!("  {}", team_name);
            }

            if let VoteType::Formal { raffle_id, .. } = vote.vote_type() {
                if let Some(raffle) = budget_system.state.raffles.get(&raffle_id) {
                    if let VoteParticipation::Formal { counted, uncounted } = vote.participation() {
                        println!("\nCounted seats:");
                        for &team_id in counted {
                            if let Some(team) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                                println!("  {} (+{} points)", team.name(), config.counted_vote_points);
                            }
                        }

                        println!("\nUncounted seats:");
                        for &team_id in uncounted {
                            if let Some(team) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                                println!("  {} (+{} points)", team.name(), config.uncounted_vote_points);
                            }
                        }
                    }
                } else {
                    println!("\nAssociated raffle not found. Cannot display seat breakdowns.");
                }
            } else {
                println!("\nThis is an informal vote, no counted/uncounted breakdown available.");
            }

            println!("\nNote: Detailed vote counts are not available for historical votes.");
        },
        ScriptCommand::ImportHistoricalRaffle { 
            proposal_name, 
            initiation_block, 
            randomness_block, 
            team_order, 
            excluded_teams,
            total_counted_seats, 
            max_earner_seats 
        } => {
            let (raffle_id, raffle) = budget_system.import_historical_raffle(
                &proposal_name,
                initiation_block,
                randomness_block,
                team_order.clone(),
                excluded_teams.clone(),
                total_counted_seats.or(Some(budget_system.config.default_total_counted_seats)),
                max_earner_seats.or(Some(budget_system.config.default_max_earner_seats)),
            ).await?;

            println!("Imported historical raffle for proposal '{}' (Raffle ID: {})", proposal_name, raffle_id);
            println!("Randomness: {}", raffle.config().block_randomness());

            // Print excluded teams
            if let Some(excluded) = excluded_teams {
                println!("Excluded teams: {:?}", excluded);
            }

            // Print ballot ID ranges for each team
            for snapshot in raffle.team_snapshots() {
                let tickets: Vec<_> = raffle.tickets().iter()
                    .filter(|t| t.team_id() == snapshot.id())
                    .collect();
                
                if !tickets.is_empty() {
                    let start = tickets.first().unwrap().index();
                    let end = tickets.last().unwrap().index();
                    println!("Team '{}' ballot range: {} - {}", snapshot.name(), start, end);
                }
            }

            // Print raffle results
            if let Some(result) = raffle.result() {
                println!("Counted seats:");
                println!("Earner seats:");
                let mut earner_count = 0;
                for &team_id in result.counted() {
                    if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                        if let TeamStatus::Earner { .. } = snapshot.status() {
                            earner_count += 1;
                            let best_score = raffle.tickets().iter()
                                .filter(|t| t.team_id() == team_id)
                                .map(|t| t.score())
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name(), best_score);
                        }
                    }
                }
                println!("Supporter seats:");
                for &team_id in result.counted() {
                    if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                        if let TeamStatus::Supporter = snapshot.status() {
                            let best_score = raffle.tickets().iter()
                                .filter(|t| t.team_id() == team_id)
                                .map(|t| t.score())
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name(), best_score);
                        }
                    }
                }
                println!("Total counted seats: {} (Earners: {}, Supporters: {})", 
                         result.counted().len(), earner_count, result.counted().len() - earner_count);

                println!("Uncounted seats:");
                println!("Earner seats:");
                for &team_id in result.uncounted() {
                    if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                        if let TeamStatus::Earner { .. } = snapshot.status() {
                            let best_score = raffle.tickets().iter()
                                .filter(|t| t.team_id() == team_id)
                                .map(|t| t.score())
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name(), best_score);
                        }
                    }
                }
                println!("Supporter seats:");
                for &team_id in result.uncounted() {
                    if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                        if let TeamStatus::Supporter = snapshot.status() {
                            let best_score = raffle.tickets().iter()
                                .filter(|t| t.team_id() == team_id)
                                .map(|t| t.score())
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name(), best_score);
                        }
                    }
                }
            } else {
                println!("Raffle result not available");
            }
        },
        ScriptCommand::ChangeTeamStatus { team_name, new_status, trailing_monthly_revenue } => {
            let team_id = budget_system.get_team_id_by_name(&team_name)
                .ok_or_else(|| format!("Team not found: {}", team_name))?;
            
            let new_status = match new_status.to_lowercase().as_str() {
                "earner" => {
                    let revenue = trailing_monthly_revenue
                        .ok_or("Trailing monthly revenue is required for Earner status")?;
                    TeamStatus::Earner { trailing_monthly_revenue: revenue }
                },
                "supporter" => TeamStatus::Supporter,
                "inactive" => TeamStatus::Inactive,
                _ => return Err(format!("Invalid status: {}", new_status).into()),
            };

            budget_system.update_team_status(team_id, &new_status)?;
            
            println!("Changed status of team '{}' to {:?}", team_name, new_status);
        },
        ScriptCommand::PrintTeamReport => {
            let report = budget_system.print_team_report();
            println!("{}", report);
        },
        ScriptCommand::PrintEpochState => {
            match budget_system.print_epoch_state() {
                Ok(report) => println!("{}", report),
                Err(e) => println!("Error printing epoch state: {}", e),
            }
        },
        ScriptCommand::PrintTeamVoteParticipation { team_name, epoch_name } => {
            match budget_system.print_team_vote_participation(&team_name, epoch_name.as_deref()) {
                Ok(report) => println!("{}", report),
                Err(e) => println!("Error printing team vote participation: {}", e),
            }
        },
        ScriptCommand::CloseProposal { proposal_name, resolution } => {
            let proposal_id = budget_system.get_proposal_id_by_name(&proposal_name)
                .ok_or_else(|| format!("Proposal not found: {}", proposal_name))?;
            
            let resolution = match resolution.to_lowercase().as_str() {
                "approved" => Resolution::Approved,
                "rejected" => Resolution::Rejected,
                "invalid" => Resolution::Invalid,
                "duplicate" => Resolution::Duplicate,
                "retracted" => Resolution::Retracted,
                _ => return Err(format!("Invalid resolution type: {}", resolution).into()),
            };
        
            budget_system.close_with_reason(proposal_id, &resolution)?;
            println!("Closed proposal '{}' with resolution: {:?}", proposal_name, resolution);
        },
        ScriptCommand::CreateRaffle { proposal_name, block_offset, excluded_teams } => {
            println!("Preparing raffle for proposal: {}", proposal_name);

            // PREPARATION PHASE
            let (raffle_id, tickets) = budget_system.prepare_raffle(&proposal_name, excluded_teams.clone(), &config)?;

            println!("Generated RaffleTickets:");
            for (team_name, start, end) in budget_system.group_tickets_by_team(&tickets) {
                println!("  {} ballot range [{}..{}]", team_name, start, end);
            }

            if let Some(excluded) = excluded_teams {
                println!("Excluded teams: {:?}", excluded);
            }

            let current_block = budget_system.ethereum_service.get_current_block().await?;
            println!("Current block number: {}", current_block);

            let initiation_block = current_block;

            let target_block = current_block + block_offset.unwrap_or(config.future_block_offset);
            println!("Target block for randomness: {}", target_block);

            // Wait for target block
            println!("Waiting for target block...");
            let mut last_observed_block = current_block;
            while budget_system.ethereum_service.get_current_block().await? < target_block {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let new_block = budget_system.ethereum_service.get_current_block().await?;
                if new_block != last_observed_block {
                    println!("Latest observed block: {}", new_block);
                    last_observed_block = new_block;
                }
            }

            // FINALIZATION PHASE
            let randomness = budget_system.ethereum_service.get_randomness(target_block).await?;
            println!("Block randomness: {}", randomness);
            println!("Etherscan URL: https://etherscan.io/block/{}#consensusinfo", target_block);

            let raffle = budget_system.finalize_raffle(raffle_id, initiation_block, target_block, randomness).await?;

            // Print results (similar to ImportHistoricalRaffle)
            println!("Raffle results for proposal '{}' (Raffle ID: {})", proposal_name, raffle_id);

            // Print raffle results
            if let Some(result) = raffle.result() {
                println!("**Counted voters:**");
                println!("Earner teams:");
                let mut earner_count = 0;
                for &team_id in result.counted() {
                    if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                        if let TeamStatus::Earner { .. } = snapshot.status() {
                            earner_count += 1;
                            let best_score = raffle.tickets().iter()
                                .filter(|t| t.team_id() == team_id)
                                .map(|t| t.score())
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name(), best_score);
                        }
                    }
                }
                println!("Supporter teams:");
                for &team_id in result.counted() {
                    if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                        if let TeamStatus::Supporter = snapshot.status() {
                            let best_score = raffle.tickets().iter()
                                .filter(|t| t.team_id() == team_id)
                                .map(|t| t.score())
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name(), best_score);
                        }
                    }
                }
                println!("Total counted voters: {} (Earners: {}, Supporters: {})", 
                         result.counted().len(), earner_count, result.counted().len() - earner_count);

                println!("**Uncounted voters:**");
                println!("Earner teams:");
                for &team_id in result.uncounted() {
                    if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                        if let TeamStatus::Earner { .. } = snapshot.status() {
                            let best_score = raffle.tickets().iter()
                                .filter(|t| t.team_id() == team_id)
                                .map(|t| t.score())
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name(), best_score);
                        }
                    }
                }
                println!("Supporter teams:");
                for &team_id in result.uncounted() {
                    if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                        if let TeamStatus::Supporter = snapshot.status() {
                            let best_score = raffle.tickets().iter()
                                .filter(|t| t.team_id() == team_id)
                                .map(|t| t.score())
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name(), best_score);
                        }
                    }
                }
            } else {
                println!("Raffle result not available");
            }
        },
        ScriptCommand::CreateAndProcessVote { proposal_name, counted_votes, uncounted_votes, vote_opened, vote_closed } => {
            println!("Executing CreateAndProcessVote command for proposal: {}", proposal_name);
            match budget_system.create_and_process_vote(
                &proposal_name,
                counted_votes,
                uncounted_votes,
                vote_opened,
                vote_closed
            ) {
                Ok(report) => {
                    println!("Vote processed successfully for proposal: {}", proposal_name);
                    println!("Vote report:\n{}", report);
                
                    // Print point credits
                    if let Some(vote_id) = budget_system.state.votes.values()
                        .find(|v| v.proposal_id() == budget_system.get_proposal_id_by_name(&proposal_name).unwrap())
                        .map(|v| v.id())
                    {
                        let vote = budget_system.state.votes.get(&vote_id).unwrap();
                        
                        println!("\nPoints credited:");
                        if let VoteParticipation::Formal { counted, uncounted } = &vote.participation() {
                            for &team_id in counted {
                                if let Some(team) = budget_system.state.current_state.teams.get(&team_id) {
                                    println!("  {} (+{} points)", team.name(), config.counted_vote_points);
                                }
                            }
                            for &team_id in uncounted {
                                if let Some(team) = budget_system.state.current_state.teams.get(&team_id) {
                                    println!("  {} (+{} points)", team.name(), config.uncounted_vote_points);
                                }
                            }
                        }
                    } else {
                        println!("Warning: Vote not found after processing");
                    }
                },
                Err(e) => {
                    println!("Error: Failed to process vote for proposal '{}'. Reason: {}", proposal_name, e);
                }
            }
        },
        ScriptCommand::GenerateReportsForClosedProposals { epoch_name } => {
            let epoch_id = budget_system.get_epoch_id_by_name(&epoch_name)
                .ok_or_else(|| format!("Epoch not found: {}", epoch_name))?;
            
            let closed_proposals: Vec<_> = budget_system.get_proposals_for_epoch(epoch_id)
                .into_iter()
                .filter(|p| p.is_closed())
                .collect();

            for proposal in closed_proposals {
                match budget_system.generate_and_save_proposal_report(proposal.id(), &epoch_name) {
                    Ok(file_path) => println!("Report generated for proposal '{}' at {:?}", proposal.title(), file_path),
                    Err(e) => println!("Failed to generate report for proposal '{}': {}", proposal.title(), e),
                }
            }
        },
        ScriptCommand::GenerateReportForProposal { proposal_name } => {
            let current_epoch = budget_system.get_current_epoch()
                .ok_or("No active epoch")?;
            
            let proposal = budget_system.get_proposals_for_epoch(current_epoch.id())
                .into_iter()
                .find(|p| p.name_matches(&proposal_name))
                .ok_or_else(|| format!("Proposal not found in current epoch: {}", proposal_name))?;

            match budget_system.generate_and_save_proposal_report(proposal.id(), &current_epoch.name()) {
                Ok(file_path) => println!("Report generated for proposal '{}' at {:?}", proposal.title(), file_path),
                Err(e) => println!("Failed to generate report for proposal '{}': {}", proposal.title(), e),
            }
        },
        ScriptCommand::PrintPointReport { epoch_name } => {
            match budget_system.generate_point_report(epoch_name.as_deref()) {
                Ok(report) => {
                    println!("Point Report:");
                    println!("{}", report);
                },
                Err(e) => println!("Error generating point report: {}", e),
            }
        },
        ScriptCommand::CloseEpoch { epoch_name } => {
            let epoch_name_clone = epoch_name.clone(); // Clone here
            match budget_system.close_epoch(epoch_name.as_deref()) {
                Ok(_) => {
                    let epoch_info = epoch_name_clone.clone().unwrap_or("Active epoch".to_string());
                    println!("Successfully closed epoch: {}", epoch_info);
                    if let Some(epoch) = budget_system.state.epochs.values().find(|e| e.name() == epoch_name_clone.as_deref().unwrap_or("")) {
                        if let Some(reward) = epoch.reward() {
                            println!("Rewards allocated:");
                            for (team_id, team_reward) in epoch.team_rewards() {
                                if let Some(team) = budget_system.state.current_state.teams.get(team_id) {
                                    println!("  {}: {} {} ({:.2}%)", team.name(), team_reward.amount(), reward.token(), team_reward.percentage() * 100.0);
                                }
                            }
                        } else {
                            println!("No rewards were set for this epoch.");
                        }
                    }
                },
                Err(e) => println!("Failed to close epoch: {}", e),
            }
        },
        ScriptCommand::GenerateEndOfEpochReport { epoch_name } => {
            budget_system.generate_end_of_epoch_report(&epoch_name)?;
            println!("Generated End of Epoch Report for epoch: {}", epoch_name);
        },

    }
    Ok(())
}


// Helper function to escape special characters for MarkdownV2
fn escape_markdown(text: &str) -> String {
    let special_chars = ['_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!'];
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if special_chars.contains(&c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

fn clean_file_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c
        })
        .collect()
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' => c,
            _ => '_'
        })
        .collect()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    // Load .env file
    dotenv().expect(".env file not found");
    let config = AppConfig::new()?;

    // Ensure the directory exists
    if let Some(parent) = Path::new(&config.state_file).parent() {
        fs::create_dir_all(parent)?;
    }

    // Create the EthereumService
    let ethereum_service = Arc::new(EthereumService::new(&config.ipc_path, config.future_block_offset).await?);

    // Initialize or load the BudgetSystem
    let mut budget_system = match BudgetSystem::load_from_file(&config.state_file, config.clone(), ethereum_service.clone()).await {
        Ok(system) => {
            println!("Loaded existing state from {}", &config.state_file);
            system
        },
        Err(e) => {
            println!("Failed to load existing state from {}: {}", &config.state_file, e);
            println!("Creating a new BudgetSystem.");
            BudgetSystem::new(config.clone(), ethereum_service.clone()).await?
        },
    };

    // Read and execute the script
    if Path::new(&config.script_file).exists() {
        let script_content = fs::read_to_string(&config.script_file)?;
        let script: Vec<ScriptCommand> = serde_json::from_str(&script_content)?;
        
        for command in script {
            if let Err(e) = execute_command(&mut budget_system, command, &config).await {
                error!("Error executing command: {}", e);
            }
        }
        println!("Script execution completed.");
    } else {
        println!("No script file found at {}. Skipping script execution.", &config.script_file);
    }

    // Save the current state
    match budget_system.save_state() {
        Ok(_) => info!("Saved current state to {}", &config.state_file),
        Err(e) => error!("Failed to save state to {}: {}", &config.state_file, e),
    }

    let (command_sender, command_receiver) = mpsc::channel(100);
    
    spawn_command_executor(budget_system, command_receiver);

    let bot = Bot::new(&config.telegram.token);
    let telegram_bot = TelegramBot::new(bot, command_sender);
    
    println!("Bot is running...");
    telegram_bot.run().await;

    Ok(())
    
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::TempDir;
    use crate::app_config::{AppConfig, TelegramConfig};

    struct MockEthereumService;

    #[async_trait::async_trait]
    impl EthereumServiceTrait for MockEthereumService {
        async fn get_current_block(&self) -> Result<u64, Box<dyn std::error::Error>> {
            Ok(12345)
        }

        async fn get_randomness(&self, block_number: u64) -> Result<String, Box<dyn std::error::Error>> {
            Ok(format!("mock_randomness_for_block_{}", block_number))
        }

        async fn get_raffle_randomness(&self) -> Result<(u64, u64, String), Box<dyn std::error::Error>> {
            Ok((12345, 12355, "mock_randomness".to_string()))
        }
    }

    // Helper function to create a test BudgetSystem
    async fn create_test_budget_system() -> BudgetSystem {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig {
            state_file: temp_dir.path().join("test_state.json").to_str().unwrap().to_string(),
            ipc_path: temp_dir.path().join("test_reth.ipc").to_str().unwrap().to_string(),
            future_block_offset: 10,
            script_file: temp_dir.path().join("test_script.json").to_str().unwrap().to_string(),
            default_total_counted_seats: 7,
            default_max_earner_seats: 5,
            default_qualified_majority_threshold: 0.7,
            counted_vote_points: 5,
            uncounted_vote_points: 2,
            telegram: TelegramConfig {
                chat_id: "test_chat_id".to_string(),
                token: "test_token".to_string(),
            },
        };
        let ethereum_service = Arc::new(MockEthereumService);
        BudgetSystem::new(config, ethereum_service).await.unwrap()
    }

    // Helper function to create and activate an epoch
    async fn create_active_epoch(budget_system: &mut BudgetSystem, name: &str, duration_days: i64) -> Uuid {
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(duration_days);
        let epoch_id = budget_system.create_epoch(name, start_date, end_date).unwrap();
        budget_system.activate_epoch(epoch_id).unwrap();
        epoch_id
    }

    #[tokio::test]
    async fn test_save_and_load_state() {
        // Create a temporary directory for this test
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("test_state.json");

        // Get a default test budget system
        let mut budget_system = create_test_budget_system().await;

        // Modify the state_file in the config
        budget_system.config.state_file = state_file.to_str().unwrap().to_string();

        // Create an epoch and add a team
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(30);
        budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
        budget_system.add_team("Test Team".to_string(), "Representative".to_string(), Some(vec![1000, 2000, 3000])).unwrap();

        // Save the state
        budget_system.save_state().unwrap();

        // Load the state into a new BudgetSystem
        let ethereum_service = Arc::new(MockEthereumService);
        let loaded_system = BudgetSystem::load_from_file(
            &budget_system.config.state_file,
            budget_system.config.clone(),
            ethereum_service
        ).await.unwrap();

        // Verify the loaded state
        assert_eq!(loaded_system.state.epochs.len(), 1);
        assert_eq!(loaded_system.state.current_state.teams.len(), 1);
    }

    #[tokio::test]
    async fn test_create_epoch() {
        let mut budget_system = create_test_budget_system().await;
        let _epoch_id = create_active_epoch(&mut budget_system, "Test Epoch", 30).await;
        
        let epoch = budget_system.get_current_epoch().unwrap();
        assert_eq!(epoch.name(), "Test Epoch");
        assert!(epoch.is_active());
    }
}