use super::*;
use pretty_assertions::assert_eq;

#[test]
fn blocked_thread_navigation_argument_policy_is_mode_specific() {
    let cases = [
        (
            DirectInputMode::ParentOwned,
            SlashCommand::Resume,
            "saved-thread",
            true,
        ),
        (
            DirectInputMode::ActiveWriterReadOnly,
            SlashCommand::Resume,
            "saved-thread",
            true,
        ),
        (
            DirectInputMode::ParentOwned,
            SlashCommand::Fork,
            "copy",
            false,
        ),
        (
            DirectInputMode::ActiveWriterReadOnly,
            SlashCommand::Fork,
            "copy",
            true,
        ),
        (
            DirectInputMode::ActiveWriterReadOnly,
            SlashCommand::New,
            "named",
            false,
        ),
        (
            DirectInputMode::ParentOwned,
            SlashCommand::Export,
            "transcript.md",
            true,
        ),
        (
            DirectInputMode::ActiveWriterReadOnly,
            SlashCommand::Export,
            "transcript.md",
            true,
        ),
    ];

    for (mode, command, args, expected) in cases {
        assert_eq!(
            blocked_thread_command_is_allowed(mode, command, args),
            expected,
            "mode={mode:?}, command={command:?}, args={args:?}"
        );
    }
}
