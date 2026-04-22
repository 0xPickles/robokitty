
# RoboKitty Usage Guide

This guide provides detailed instructions and examples for using RoboKitty through its two interfaces: the Command-Line Interface (CLI) and the Telegram Bot.

## 1. Command-Line Interface (CLI)

The CLI (`robokitty_cli`) is the most powerful interface, intended for administrators, scripting, and complex operations.

**General Syntax:** `robokitty_cli <command> <subcommand> [options]`

To see all available commands, run `robokitty_cli --help`. To see options for a specific command, run `robokitty_cli <command> --help`.

### Team Management (`team`)

-   **Add a Team:**
    ```bash
    # Add a supporter team
    robokitty_cli team add --name "ySupport" --representative "@user" --address "0x..."

    # Add an earner team with trailing 3 months of revenue
    robokitty_cli team add --name "yLockers" --representative "@dev" --revenue "10000,12000,11000"
    ```

-   **Update a Team:**
    ```bash
    # Change a team's name and status
    robokitty_cli team update "ySupport" --new-name "ySupport Crew" --status "Inactive"

    # Update an earner's revenue
    robokitty_cli team update "yLockers" --revenue "11000,13000,12500"
    ```

### Epoch Management (`epoch`)

-   **Create an Epoch:** Dates must be in RFC3339 format (UTC).
    ```bash
    robokitty_cli epoch create E5 "2025-05-01T00:00:00Z" "2025-07-31T23:59:59Z"
    ```

-   **Activate an Epoch:** Sets the newly created epoch as the active one for new proposals.
    ```bash
    robokitty_cli epoch activate E5
    ```

-   **Set Epoch Reward:**
    ```bash
    robokitty_cli epoch set-reward YFI 18.0
    ```

-   **Close an Epoch:** Calculates final team rewards and marks the epoch as `Closed`. Can only be done if there are no open proposals.
    ```bash
    robokitty_cli epoch close E5
    ```

### Proposal Management (`proposal`)

-   **Add a Proposal:**
    ```bash
    # Simple proposal without a budget
    robokitty_cli proposal add --title "Community Poll" --url "https://snapshot.org/..."

    # Proposal with a budget request
    robokitty_cli proposal add --title "New Feature" --team "yLockers" --amounts "USDC:50000,YFI:2.5" --start "2025-05-01" --loan "false"
    ```

-   **Update a Proposal:**
    ```bash
    robokitty_cli proposal update "New Feature" --url "https://new-link.com"
    ```

-   **Close a Proposal:**
    ```bash
    robokitty_cli proposal close "New Feature" Approved
    ```

-   **Log a Payment:** Marks one or more approved proposals as paid.
    ```bash
    robokitty_cli proposal pay "Proposal A,Proposal B" --tx "0x..." --date "2025-05-20"
    ```

### Raffle & Vote Management

-   **Create a Raffle:** Initiates the on-chain raffle process for a proposal.
    ```bash
    robokitty_cli raffle create "New Feature"
    ```

-   **Process a Vote:** Manually records the results of a vote. This is useful for importing historical data or when votes happen off-platform.
    ```bash
    robokitty_cli vote process "New Feature" --counted "TeamA:Yes,TeamB:No" --uncounted "TeamC:Yes"
    ```

### Reporting (`report`)

-   **Generate Team Report:**
    ```bash
    robokitty_cli report team
    robokitty_cli report team Dinobots
    ```

-   **Generate Unpaid Requests Report:**
    ```bash
    robokitty_cli report unpaid-requests --output-path reports/unpaid.json
    ```

-   **Generate All Epochs Summary:**
    ```bash
    robokitty_cli report all-epochs --only-closed --output-path reports/summary.md
    ```

### Run a Script

Execute a series of commands from a JSON file.
```bash
robokitty_cli run-script --script-file-path "my_setup_script.json"
```

---

## 2. Telegram Bot

The Telegram bot (`robokitty_bot`) offers a conversational interface for many common commands. It's ideal for quick actions and status checks.

**General Syntax:** `/command key:value [key2:value2 ...]`

### Common Commands

-   `/help`: Displays a list of all available bot commands.
-   `/print_team_report`: Shows a detailed report of all registered teams.
-   `/print_epoch_state`: Displays the status of the currently active epoch, including open proposals.

### Examples

-   **Add a Team:**
    ```
    /add_team name:ySupport rep:@user
    /add_team name:yLockers rep:@dev rev:10000,12000,11000
    ```
-   **Update a Team:**
    ```
    /update_team team:ySupport status:Inactive
    ```
-   **Add a Proposal:**
    ```
    /add_proposal title:New Feature url:http://... team:yLockers amounts:USDC:50000
    ```
-   **Create a Raffle:**
    ```
    /create_raffle name:New Feature
    ```
    The bot will post live updates as it waits for the target block and finalizes the results.

-   **Process a Vote:**
    ```
    /process_vote name:New Feature counted:TeamA:Yes,TeamB:No uncounted:TeamC:Yes
    ```

-   **Log a Payment:**
    ```
    /log_payment tx:0x... date:2025-05-20 proposals:Proposal A,Proposal B
    ```
