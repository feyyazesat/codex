use crate::chatwidget::DirectInputMode;
use crate::slash_command::SlashCommand;

pub(super) fn blocked_thread_command_is_allowed(
    mode: DirectInputMode,
    command: SlashCommand,
    args: &str,
) -> bool {
    if command == SlashCommand::Export {
        return true;
    }

    let command_is_allowed = matches!(
        command,
        SlashCommand::Feedback
            | SlashCommand::New
            | SlashCommand::Clear
            | SlashCommand::Resume
            | SlashCommand::App
            | SlashCommand::Side
            | SlashCommand::Btw
            | SlashCommand::Agents
            | SlashCommand::MultiAgents
            | SlashCommand::Vim
            | SlashCommand::Keymap
            | SlashCommand::ElevateSandbox
            | SlashCommand::SandboxReadRoot
            | SlashCommand::Experimental
            | SlashCommand::Memories
            | SlashCommand::Quit
            | SlashCommand::Exit
            | SlashCommand::Logout
            | SlashCommand::Copy
            | SlashCommand::Raw
            | SlashCommand::Diff
            | SlashCommand::Mention
            | SlashCommand::Skills
            | SlashCommand::Import
            | SlashCommand::Hooks
            | SlashCommand::Status
            | SlashCommand::Usage
            | SlashCommand::Ide
            | SlashCommand::DebugConfig
            | SlashCommand::Title
            | SlashCommand::Statusline
            | SlashCommand::Theme
            | SlashCommand::Pets
            | SlashCommand::Ps
            | SlashCommand::Stop
            | SlashCommand::MemoryDrop
            | SlashCommand::MemoryUpdate
            | SlashCommand::Mcp
            | SlashCommand::Apps
            | SlashCommand::Plugins
            | SlashCommand::Rollout
    ) || mode == DirectInputMode::ActiveWriterReadOnly
        && command == SlashCommand::Fork;
    let arguments_are_allowed = args.is_empty()
        || command == SlashCommand::Resume
        || mode == DirectInputMode::ActiveWriterReadOnly && command == SlashCommand::Fork;
    command_is_allowed && arguments_are_allowed
}

#[cfg(test)]
#[path = "direct_input_policy_tests.rs"]
mod tests;
